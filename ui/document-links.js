(function (root, factory) {
	const api = factory();
	if (typeof module === "object" && module.exports) module.exports = api;
	root.StillwriteDocumentLinks = api;
})(typeof globalThis !== "undefined" ? globalThis : window, function () {
	"use strict";

	function normalizePath(path) {
		const value = String(path || "").replaceAll("\\", "/");
		const prefix = value.match(/^[A-Za-z]:\//)?.[0] || (value.startsWith("/") ? "/" : "");
		const parts = [];
		value.slice(prefix.length).split("/").forEach((part) => {
			if (!part || part === ".") return;
			if (part === "..") parts.pop();
			else parts.push(part);
		});
		return prefix + parts.join("/");
	}

	function basename(path) {
		return normalizePath(path).split("/").pop() || "";
	}

	function collectDocuments(nodes, rootPath) {
		const root = normalizePath(rootPath).replace(/\/$/, "");
		const documents = [];
		function visit(items) {
			(items || []).forEach((node) => {
				if (node.is_dir) visit(node.children);
				else if (/\.(?:md|markdown)$/i.test(node.path || node.name || "")) {
					const path = normalizePath(node.path);
					documents.push({
						path,
						relative: path.startsWith(`${root}/`) ? path.slice(root.length + 1) : basename(path),
						name: node.name || basename(path),
					});
				}
			});
		}
		visit(nodes);
		return documents;
	}

	function buildIndex(nodes, rootPath) {
		const documents = collectDocuments(nodes, rootPath);
		const byPath = new Map();
		const aliases = new Map();
		const stems = new Map();
		documents.forEach((doc) => {
			byPath.set(doc.path, doc);
			byPath.set(normalizePath(doc.relative), doc);
			// Keep the absolute form as an alias too.  This is important for
			// plain-text paths copied from search results, logs, or another app.
			// Markdown links usually contain a relative path, while prose often
			// contains `/workspace/notes/file.md` verbatim.
			const aliasesForDocument = [
				doc.path,
				doc.path.replaceAll("/", "\\"),
				encodeURI(doc.path),
				`file://${doc.path}`,
				`file://${encodeURI(doc.path)}`,
				doc.relative,
				doc.relative.replaceAll("/", "\\"),
				encodeURI(doc.relative),
				doc.name,
			];
			for (const alias of aliasesForDocument) {
				if (alias) aliases.set(alias, doc);
			}
			const stem = doc.name.replace(/\.(?:md|markdown)$/i, "");
			if (stem.length >= 2) {
				const matches = stems.get(stem) || [];
				matches.push(doc);
				stems.set(stem, matches);
			}
		});
		stems.forEach((matches, stem) => {
			if (matches.length === 1) aliases.set(stem, matches[0]);
		});
		return {
			documents,
			byPath,
			aliases: [...aliases.entries()]
				.map(([label, doc]) => ({ label, path: doc.path }))
				.sort((a, b) => b.label.length - a.label.length),
		};
	}

	function safeDecode(value) {
		try {
			return decodeURIComponent(value);
		} catch (_) {
			return value;
		}
	}

	function resolveInternalHref(href, currentFile, rootPath, index) {
		let value = safeDecode(String(href || "").trim())
			.replace(/^<|>$/g, "")
			.trim();
		if (
			!value ||
			value.startsWith("#") ||
			/^(?:https?:|mailto:|tel:|data:|javascript:)/i.test(value) ||
			value.startsWith("//")
		)
			return null;
		// A full path may have been pasted as a file URL.  Convert it back to
		// the filesystem spelling used by the workspace tree before resolving.
		if (/^file:\/\//i.test(value)) {
			value = value.replace(/^file:\/\/(?:localhost)?/i, "");
			if (/^\/[A-Za-z]:[\\/]/.test(value)) value = value.slice(1);
		}
		value = value.split(/[?#]/, 1)[0];
		const root = normalizePath(rootPath).replace(/\/$/, "");
		const current = normalizePath(currentFile);
		const base = current.includes("/") ? current.slice(0, current.lastIndexOf("/")) : root;
		const absolute = value.startsWith("/") || /^[A-Za-z]:[\\/]/.test(value);
		const candidates = absolute
			? [normalizePath(value)]
			: [
					normalizePath(`${base}/${value}`),
					normalizePath(`${root}/${value}`),
					normalizePath(value),
				];
		for (const candidate of candidates) {
			const doc = index.byPath.get(candidate);
			if (doc) return doc;
		}
		return null;
	}

	function trimUrl(url) {
		return url.replace(/[.,;:!?，。；：！？、）》】]+$/u, "");
	}

	function findNextLink(text, offset, index) {
		const rest = text.slice(offset);
		const urlMatch = rest.match(/https?:\/\/[^\s<>，。；：！？、（）《》【】“”‘’]+/i);
		let best = urlMatch
			? {
					start: offset + urlMatch.index,
					label: trimUrl(urlMatch[0]),
					href: trimUrl(urlMatch[0]),
					type: "external",
				}
			: null;

		for (const alias of index.aliases) {
			const found = text.indexOf(alias.label, offset);
			if (found < 0) continue;
			if (
				best &&
				(found > best.start || (found === best.start && alias.label.length <= best.label.length))
			)
				continue;
			if (/^[A-Za-z0-9_-]+$/.test(alias.label)) {
				const before = text[found - 1] || "";
				const after = text[found + alias.label.length] || "";
				if (/[A-Za-z0-9_-]/.test(before) || /[A-Za-z0-9_-]/.test(after)) continue;
			}
			best = {
				start: found,
				label: alias.label,
				href: alias.path,
				type: "internal",
			};
		}
		return best;
	}

	function segmentText(text, index) {
		const segments = [];
		let offset = 0;
		while (offset < text.length) {
			const link = findNextLink(text, offset, index);
			if (!link || !link.label) break;
			if (link.start > offset) segments.push({ text: text.slice(offset, link.start) });
			segments.push(link);
			offset = link.start + link.label.length;
		}
		if (offset < text.length) segments.push({ text: text.slice(offset) });
		return segments;
	}

	return { buildIndex, normalizePath, resolveInternalHref, segmentText };
});
