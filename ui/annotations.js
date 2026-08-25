(function (root, factory) {
	const api = factory();
	if (typeof module === "object" && module.exports) module.exports = api;
	root.StillwriteAnnotations = api;
})(typeof globalThis !== "undefined" ? globalThis : window, function () {
	"use strict";

	const FORMAT_HEADER = "<!-- stillwrite-annotations:v1 -->";
	const ITEM_END = "<!-- /stillwrite-annotation -->";
	const QUOTE_START = "<!-- stillwrite-quote -->";
	const QUOTE_END = "<!-- /stillwrite-quote -->";

	function encodeMeta(value) {
		const json = JSON.stringify(value);
		if (typeof Buffer !== "undefined")
			return Buffer.from(json, "utf8").toString("base64url");
		const bytes = new TextEncoder().encode(json);
		let binary = "";
		bytes.forEach((byte) => {
			binary += String.fromCharCode(byte);
		});
		return btoa(binary)
			.replaceAll("+", "-")
			.replaceAll("/", "_")
			.replace(/=+$/g, "");
	}

	function decodeMeta(value) {
		try {
			if (typeof Buffer !== "undefined")
				return JSON.parse(Buffer.from(value, "base64url").toString("utf8"));
			const padded = value.replaceAll("-", "+").replaceAll("_", "/");
			const binary = atob(padded.padEnd(Math.ceil(padded.length / 4) * 4, "="));
			const bytes = Uint8Array.from(binary, (char) => char.charCodeAt(0));
			return JSON.parse(new TextDecoder().decode(bytes));
		} catch (_) {
			return null;
		}
	}

	function newId() {
		if (globalThis.crypto?.randomUUID) return globalThis.crypto.randomUUID();
		return `note-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 9)}`;
	}

	function normalizeItem(item, fallbackTime = "") {
		return {
			id: String(item.id || newId()),
			kind: item.kind === "paragraph" || item.kind === "document" ? item.kind : "selection",
			start: Number.isInteger(item.start) ? item.start : -1,
			end: Number.isInteger(item.end) ? item.end : -1,
			quote: String(item.quote || "").trim(),
			note: String(item.note || "").trim(),
			createdAt: String(item.createdAt || fallbackTime || ""),
			updatedAt: String(item.updatedAt || item.createdAt || fallbackTime || ""),
		};
	}

	function parseQuote(block) {
		const match = block.match(
			/<!-- stillwrite-quote -->\s*\n([\s\S]*?)\n<!-- \/stillwrite-quote -->/,
		);
		if (!match) return { quote: "", rest: block.trim() };
		const lines = match[1].split("\n");
		if (/^> 原文（.+）：\s*$/.test(lines[0] || "")) lines.shift();
		const quote = lines
			.map((line) => line.replace(/^> ?/, ""))
			.join("\n")
			.trim();
		const rest = `${block.slice(0, match.index)}${block.slice(match.index + match[0].length)}`.trim();
		return { quote, rest };
	}

	function parse(body, fallbackTime = "") {
		const source = String(body || "").trim();
		if (!source) return [];
		if (!source.includes(FORMAT_HEADER)) {
			return [
				normalizeItem(
					{
						id: "legacy-document-note",
						kind: "document",
						start: -1,
						end: -1,
						quote: "",
						note: source,
					},
					fallbackTime,
				),
			];
		}

		const items = [];
		const pattern = /<!-- stillwrite-annotation:([A-Za-z0-9_-]+) -->\s*\n([\s\S]*?)\n<!-- \/stillwrite-annotation -->/g;
		let match;
		while ((match = pattern.exec(source))) {
			const meta = decodeMeta(match[1]);
			if (!meta) continue;
			const parsed = parseQuote(match[2]);
			items.push(
				normalizeItem(
					{ ...meta, quote: parsed.quote, note: parsed.rest },
					fallbackTime,
				),
			);
		}
		return items;
	}

	function quoteLabel(kind) {
		if (kind === "paragraph") return "段落";
		if (kind === "document") return "全文";
		return "字句";
	}

	function serialize(items) {
		const valid = items
			.map((item) => normalizeItem(item))
			.filter((item) => item.note || item.quote);
		if (!valid.length) return "";
		const blocks = valid.map((item) => {
			const meta = encodeMeta({
				id: item.id,
				kind: item.kind,
				start: item.start,
				end: item.end,
				createdAt: item.createdAt,
				updatedAt: item.updatedAt,
			});
			const quoted = (item.quote || "整篇文档")
				.split("\n")
				.map((line) => `> ${line}`)
				.join("\n");
			return [
				`<!-- stillwrite-annotation:${meta} -->`,
				QUOTE_START,
				`> 原文（${quoteLabel(item.kind)}）：`,
				quoted,
				QUOTE_END,
				"",
				item.note.trim(),
				ITEM_END,
			].join("\n");
		});
		return `${FORMAT_HEADER}\n\n${blocks.join("\n\n")}\n`;
	}

	function trimRange(source, start, end) {
		while (start < end && /\s/.test(source[start])) start += 1;
		while (end > start && /\s/.test(source[end - 1])) end -= 1;
		return { start, end };
	}

	function selectionOrParagraph(source, start, end) {
		const text = String(source || "");
		let range = trimRange(text, Math.max(0, start), Math.min(text.length, end));
		let kind = "selection";
		if (range.start === range.end) {
			kind = "paragraph";
			const before = text.slice(0, range.start);
			const after = text.slice(range.end);
			let blockStart = 0;
			const paragraphBreak = /\n[ \t]*\n/g;
			let previousBreak;
			while ((previousBreak = paragraphBreak.exec(before)))
				blockStart = previousBreak.index + previousBreak[0].length;
			const nextBreak = after.search(/\n\s*\n/);
			const blockEnd = nextBreak < 0 ? text.length : range.end + nextBreak;
			range = trimRange(text, blockStart, blockEnd);
		}
		return {
			kind,
			start: range.start,
			end: range.end,
			quote: text.slice(range.start, range.end),
		};
	}

	function resolveRange(source, item) {
		const text = String(source || "");
		const quote = String(item.quote || "");
		if (!quote) return { start: -1, end: -1 };
		if (
			item.start >= 0 &&
			item.end >= item.start &&
			text.slice(item.start, item.end) === quote
		)
			return { start: item.start, end: item.end };

		const matches = [];
		let offset = 0;
		while ((offset = text.indexOf(quote, offset)) >= 0) {
			matches.push(offset);
			offset += Math.max(1, quote.length);
		}
		if (!matches.length) return { start: -1, end: -1 };
		const oldStart = item.start >= 0 ? item.start : 0;
		const start = matches.reduce((best, candidate) =>
			Math.abs(candidate - oldStart) < Math.abs(best - oldStart) ? candidate : best,
		);
		return { start, end: start + quote.length };
	}

	return {
		FORMAT_HEADER,
		parse,
		serialize,
		newId,
		normalizeItem,
		selectionOrParagraph,
		resolveRange,
	};
});
