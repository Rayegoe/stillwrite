import { closeSync, openSync, readdirSync, realpathSync, readSync, statSync } from "node:fs";
import { isAbsolute, join, relative, resolve, sep } from "node:path";

const MAX_LIST_ENTRIES = 500;
const MAX_READ_BYTES = 512 * 1024;
const MAX_SEARCH_FILE_BYTES = 2 * 1024 * 1024;
const MAX_SEARCH_TOTAL_BYTES = 32 * 1024 * 1024;
const DEFAULT_SEARCH_RESULTS = 20;
const MAX_SEARCH_RESULTS = 50;
const IGNORED_DIRECTORIES = new Set([".git", "node_modules", "target"]);
let cachedWorkspaceRoot;

function workspaceRoot() {
	if (cachedWorkspaceRoot) return cachedWorkspaceRoot;
	const configured = process.env.STILLWRITE_PI_WORKSPACE_ROOT;
	if (!configured) throw new Error("StillWrite Workspace is not configured");
	const root = realpathSync(configured);
	if (!statSync(root).isDirectory()) throw new Error("StillWrite Workspace is not a directory");
	cachedWorkspaceRoot = root;
	return cachedWorkspaceRoot;
}

function markdownPath(value) {
	return /\.(?:md|markdown)$/i.test(value);
}

function normalizedRelative(value = ".") {
	if (typeof value !== "string" || value.includes("\0")) {
		throw new Error("Workspace path is invalid");
	}
	const normalized = value.replaceAll("\\", "/");
	if (
		isAbsolute(value) ||
		/^\/{2}|^[A-Za-z]:\//.test(normalized) ||
		normalized.split("/").some((part) => part === "..")
	) {
		throw new Error("Workspace path must stay inside the active Workspace");
	}
	return normalized || ".";
}

function inside(relativePath, { directory = false } = {}) {
	const root = workspaceRoot();
	const candidate = resolve(root, normalizedRelative(relativePath));
	const canonical = realpathSync(candidate);
	const rel = relative(root, canonical);
	if (rel === "" || (!rel.startsWith(`..${sep}`) && rel !== ".." && !isAbsolute(rel))) {
		if (directory && !statSync(canonical).isDirectory()) throw new Error("Workspace path is not a directory");
		return canonical;
	}
	throw new Error("Workspace path must stay inside the active Workspace");
}

function textFromBuffer(buffer) {
	return new TextDecoder("utf-8", { fatal: true }).decode(buffer);
}

function readBoundedUtf8(path, limit) {
	const descriptor = openSync(path, "r");
	try {
		const buffer = Buffer.alloc(limit + 4);
		const bytes = readSync(descriptor, buffer, 0, buffer.length, 0);
		let end = Math.min(bytes, limit);
		let text;
		while (end >= 0) {
			try {
				text = textFromBuffer(buffer.subarray(0, end));
				break;
			} catch (error) {
				if (end === 0) throw error;
				end -= 1;
			}
		}
		return { text, bytesRead: end, truncated: bytes > limit || end < bytes };
	} finally {
		closeSync(descriptor);
	}
}

function boundedText(value, limit) {
	const bytes = Buffer.byteLength(value, "utf8");
	if (bytes <= limit) return value;
	let result = "";
	for (const character of value) {
		const next = result + character;
		if (Buffer.byteLength(next, "utf8") > limit) break;
		result = next;
	}
	return `${result}\n[… output truncated …]`;
}

function result(value) {
	return { content: [{ type: "text", text: JSON.stringify(value) }], details: {} };
}

function listDirectory(path) {
	const entries = [];
	for (const entry of readdirSync(path, { withFileTypes: true })) {
		if (entry.name.startsWith(".") || (entry.isDirectory() && IGNORED_DIRECTORIES.has(entry.name))) continue;
		const child = join(path, entry.name);
		let canonical;
		try {
			canonical = realpathSync(child);
		} catch {
			continue;
		}
		const root = workspaceRoot();
		const rel = relative(root, canonical);
		if (rel === ".." || rel.startsWith(`..${sep}`) || isAbsolute(rel)) continue;
		if (entry.isDirectory()) {
			entries.push({ path: rel.split(sep).join("/"), kind: "directory" });
		} else if (entry.isFile() && markdownPath(entry.name)) {
			entries.push({ path: rel.split(sep).join("/"), kind: "markdown" });
		}
		if (entries.length >= MAX_LIST_ENTRIES) break;
	}
	entries.sort((a, b) => a.path.localeCompare(b.path));
	return result({ entries, truncated: entries.length >= MAX_LIST_ENTRIES });
}

function readWorkspace(params) {
	const path = inside(params.path);
	if (!statSync(path).isFile() || !markdownPath(path)) throw new Error("Only Markdown files can be read");
	const bounded = readBoundedUtf8(path, MAX_READ_BYTES);
	const content = bounded.truncated ? `${bounded.text}\n[… file truncated …]` : bounded.text;
	const lines = content.replace(/\r\n?/g, "\n").split("\n");
	const start = params.start_line === undefined ? 1 : Number(params.start_line);
	const requestedEnd = params.end_line === undefined ? lines.length : Number(params.end_line);
	if (!Number.isInteger(start) || start < 1 || !Number.isInteger(requestedEnd) || requestedEnd < start) {
		throw new Error("Line range must use positive integers");
	}
	const end = Math.min(requestedEnd, start + 20000, lines.length);
	const numbered = lines.slice(start - 1, end).map((line, index) => `${start + index}: ${line}`).join("\n");
	return result({ path: normalizedRelative(params.path), startLine: start, endLine: end, text: boundedText(numbered, MAX_READ_BYTES) });
}

function walkMarkdown(path, output) {
	for (const entry of readdirSync(path, { withFileTypes: true })) {
		if (entry.name.startsWith(".") || (entry.isDirectory() && IGNORED_DIRECTORIES.has(entry.name))) continue;
		const child = join(path, entry.name);
		let canonical;
		try {
			canonical = realpathSync(child);
		} catch {
			continue;
		}
		const root = workspaceRoot();
		const rel = relative(root, canonical);
		if (rel === ".." || rel.startsWith(`..${sep}`) || isAbsolute(rel)) continue;
		if (entry.isDirectory()) walkMarkdown(canonical, output);
		else if (entry.isFile() && markdownPath(entry.name)) output.push({ path: canonical, relative: rel.split(sep).join("/") });
	}
}

function searchWorkspace(params, signal) {
	if (typeof params.query !== "string" || !params.query.trim()) throw new Error("Search query must not be empty");
	const requested = Number(params.max_results ?? DEFAULT_SEARCH_RESULTS);
	const maxResults = Number.isInteger(requested) ? Math.max(1, Math.min(MAX_SEARCH_RESULTS, requested)) : DEFAULT_SEARCH_RESULTS;
	const query = params.query.normalize("NFKC").toLocaleLowerCase();
	const files = [];
	walkMarkdown(workspaceRoot(), files);
	files.sort((a, b) => a.relative.localeCompare(b.relative));
	const matches = [];
	let scannedBytes = 0;
	for (const file of files) {
		if (signal?.aborted) throw new Error("Workspace search was cancelled");
		if (matches.length >= maxResults || scannedBytes >= MAX_SEARCH_TOTAL_BYTES) break;
		try {
			const bounded = readBoundedUtf8(file.path, MAX_SEARCH_FILE_BYTES);
			const content = bounded.text;
			scannedBytes += bounded.bytesRead;
			const lines = content.replace(/\r\n?/g, "\n").split("\n");
			for (let index = 0; index < lines.length && matches.length < maxResults; index += 1) {
				if (lines[index].normalize("NFKC").toLocaleLowerCase().includes(query)) {
					matches.push({ path: file.relative, line: index + 1, snippet: boundedText(lines[index].trim(), 240) });
				}
			}
		} catch {
			// Binary/non-UTF-8 files are intentionally skipped.
		}
	}
	return result({ query: params.query, matches, scannedBytes, truncated: matches.length >= maxResults || scannedBytes >= MAX_SEARCH_TOTAL_BYTES });
}

export default function (pi) {
	pi.registerTool({
		name: "workspace_list",
		label: "List Workspace",
		description: "List Markdown files and directories in the active StillWrite Workspace.",
		parameters: {
			type: "object",
			properties: { path: { type: "string", description: "Workspace-relative directory, default ." } },
			additionalProperties: false,
		},
		async execute(_toolCallId, params, signal) {
			return listDirectory(inside(params.path ?? ".", { directory: true }));
		},
	});

	pi.registerTool({
		name: "workspace_read",
		label: "Read Workspace",
		description: "Read bounded UTF-8 Markdown content from the active StillWrite Workspace.",
		parameters: {
			type: "object",
			properties: {
				path: { type: "string", description: "Workspace-relative Markdown path" },
				start_line: { type: "integer", minimum: 1 },
				end_line: { type: "integer", minimum: 1 },
			},
			required: ["path"],
			additionalProperties: false,
		},
		async execute(_toolCallId, params) {
			return readWorkspace(params);
		},
	});

	pi.registerTool({
		name: "workspace_search",
		label: "Search Workspace",
		description: "Search bounded Markdown text in the active StillWrite Workspace.",
		parameters: {
			type: "object",
			properties: {
				query: { type: "string", minLength: 1 },
				max_results: { type: "integer", minimum: 1, maximum: MAX_SEARCH_RESULTS },
			},
			required: ["query"],
			additionalProperties: false,
		},
		async execute(_toolCallId, params, signal) {
			return searchWorkspace(params, signal);
		},
	});
}
