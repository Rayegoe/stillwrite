// Markdown 格式命令纯函数模块：不依赖 DOM。
// 输入统一为 { value, selectionStart, selectionEnd, command, options }
// 输出统一为 { value, selectionStart, selectionEnd }
// 浏览器导出 window.StillwriteMarkdownFormatting；Node 测试导出 module.exports。
"use strict";

const INLINE_COMMANDS = new Set([
	"bold",
	"italic",
	"strikethrough",
	"code",
	"link",
	"image",
]);

const LINE_COMMANDS = new Set([
	"quote",
	"list",
	"ordered-list",
	"check-list",
	"heading",
]);

function clampIndex(value, index) {
	if (typeof index !== "number" || Number.isNaN(index)) return 0;
	return Math.max(0, Math.min(value.length, Math.trunc(index)));
}

function normalizeSelection(input) {
	const start = clampIndex(input.value, input.selectionStart ?? 0);
	let end = clampIndex(input.value, input.selectionEnd ?? start);
	if (end < start) {
		const tmp = start;
		start = end;
		end = tmp;
	}
	return { start, end };
}

// 选区文本是否被某个 wrapper 完整包裹（如 ** 、* 、~~ 、` 、[..](..)）
function isWrapped(value, start, end, prefix, suffix) {
	const before = value.slice(0, start);
	const after = value.slice(end);
	const leading = before.slice(-prefix.length);
	const trailing = after.slice(0, suffix.length);
	if (leading !== prefix || trailing !== suffix) return false;
	// 防止空选区与 `**`/`*`/`~~` 自身被认为是包裹
	if (end - start === 0) return false;
	return true;
}

function wrapSelection(value, start, end, prefix, suffix, placeholder) {
	const selected = value.slice(start, end);
	const inner = selected || placeholder;
	const wrapped = `${prefix}${inner}${suffix}`;
	const nextStart = start + prefix.length;
	const nextEnd = start + prefix.length + inner.length;
	return {
		value: `${value.slice(0, start)}${wrapped}${value.slice(end)}`,
		selectionStart: nextStart,
		selectionEnd: nextEnd,
	};
}

function isLinkSelection(text) {
	return /^\[[^\]]*\]\([^)\n]*\)$/.test(text);
}

/**
 * Inline toggle：bold / italic / strikethrough / inline code。
 * 两种包裹形式都能 toggle：
 * 1. 选区包含 marker（`**abc**` 被整个选中）→ 移除 marker；
 * 2. 选区在 marker 内部（`**|abc|**`）→ 移除整个包裹；
 * 无选区 → 插入带 placeholder 的包裹并选中 placeholder。
 */
function applyInlineToggle(input, prefix, suffix, placeholder) {
	const { start, end } = normalizeSelection(input);
	const selected = input.value.slice(start, end);
	const before = input.value.slice(0, start);
	const after = input.value.slice(end);

	// 形式 1：选区本身以 marker 开头结尾（整个包裹被选中）
	if (
		selected.length > prefix.length + suffix.length &&
		selected.startsWith(prefix) &&
		selected.endsWith(suffix)
	) {
		const inner = selected.slice(prefix.length, selected.length - suffix.length);
		return {
			value: `${before}${inner}${after}`,
			selectionStart: start,
			selectionEnd: start + inner.length,
		};
	}

	// 形式 2：marker 紧贴选区外侧（`**` + 选区 + `**`）
	if (
		selected.length > 0 &&
		before.endsWith(prefix) &&
		after.startsWith(suffix)
	) {
		const newStart = start - prefix.length;
		return {
			value: `${input.value.slice(0, newStart)}${selected}${input.value.slice(
				end + suffix.length,
			)}`,
			selectionStart: newStart,
			selectionEnd: newStart + selected.length,
		};
	}

	return wrapSelection(input.value, start, end, prefix, suffix, placeholder);
}

/**
 * Inline code：单行选区用 `…`，文本内已有反引号则升为双反引号；
 * 多行选区整体包 fenced code block；若选区已是完整 fence 则移除。
 */
function applyCodeCommand(input) {
	const { start, end } = normalizeSelection(input);
	const selected = input.value.slice(start, end);

	if (selected && selected.includes("\n")) {
		// 多行选区：fenced block toggle（选区包含整个 fence 时移除）
		const fenced = /^```[^\n]*\n[\s\S]*\n```$/.test(selected);
		if (fenced) {
			const inner = selected
				.replace(/^```[^\n]*\n/, "")
				.replace(/\n```$/, "");
			return {
				value: `${input.value.slice(0, start)}${inner}${input.value.slice(end)}`,
				selectionStart: start,
				selectionEnd: start + inner.length,
			};
		}
		const wrapped = `\`\`\`\n${selected}\n\`\`\``;
		// 选中整个 fenced block，方便再次点击时移除
		return {
			value: `${input.value.slice(0, start)}${wrapped}${input.value.slice(end)}`,
			selectionStart: start,
			selectionEnd: start + wrapped.length,
		};
	}

	if (selected && isWrapped(input.value, start, end, "`", "`")) {
		return applyInlineToggle(input, "`", "`", "code");
	}
	if (selected && isWrapped(input.value, start, end, "``", "``")) {
		return applyInlineToggle(input, "``", "``", "code");
	}
	if (selected && selected.includes("`")) {
		return wrapSelection(input.value, start, end, "``", "``", selected);
	}
	return applyInlineToggle(input, "`", "`", "code");
}

/**
 * Link：有选区 → [text](https://) 并选中 URL placeholder；
 * 已是完整 link 选区 → 保持不变（destructive toggle 不做）。
 * 无选区 → [link text](https://) 选中 link text。
 */
function applyLinkCommand(input) {
	const { start, end } = normalizeSelection(input);
	const selected = input.value.slice(start, end);
	if (selected && isLinkSelection(selected)) {
		return {
			value: input.value,
			selectionStart: start,
			selectionEnd: end,
		};
	}
	if (selected) {
		// [alt](url)：URL 从 `](` 之后开始（+3）
		const urlStart = start + selected.length + 3;
		return {
			value: `${input.value.slice(0, start)}[${selected}](https://)${input.value.slice(end)}`,
			selectionStart: urlStart,
			selectionEnd: urlStart + 8,
		};
	}
	const placeholder = "link text";
	const nextStart = start + 1;
	return {
		value: `${input.value.slice(0, start)}[${placeholder}](https://)${input.value.slice(end)}`,
		selectionStart: nextStart,
		selectionEnd: nextStart + placeholder.length,
	};
}

/**
 * Remote image：有选区作为 alt → ![alt](https://)，选中 URL placeholder；
 * 无选区 → ![image](https://)，选中 URL placeholder。
 */
function applyImageCommand(input) {
	const { start, end } = normalizeSelection(input);
	const selected = input.value.slice(start, end);
	const alt = selected || "image";
	return {
		value: `${input.value.slice(0, start)}![${alt}](https://)${input.value.slice(end)}`,
		selectionStart: start + alt.length + 4,
		selectionEnd: start + alt.length + 4 + 8,
	};
}

function lineBounds(value, position) {
	const lineStart = value.lastIndexOf("\n", Math.max(0, position - 1)) + 1;
	const rawEnd = value.indexOf("\n", position);
	const lineEnd = rawEnd === -1 ? value.length : rawEnd;
	return { lineStart, lineEnd };
}

/**
 * 覆盖选区涉及的所有完整逻辑行（含选区末行若被部分选中）。
 */
function selectedLineRanges(value, start, end) {
	const ranges = [];
	let cursor = start;
	if (start === end) {
		ranges.push(lineBounds(value, start));
	} else {
		const first = lineBounds(value, start);
		let lineEnd = first.lineEnd;
		ranges.push(first);
		while (lineEnd < value.length && lineEnd < end) {
			const next = lineBounds(value, lineEnd + 1);
			ranges.push(next);
			lineEnd = next.lineEnd;
		}
	}
	return ranges;
}

const LINE_PREFIX_RE =
	/^(?:(> )|(- \[[ xX]\] )|([-+*] )|(\d+[.)] ))/;

function lineHasAnyPrefix(line) {
	return /^(?:> |[-+*] |\d+[.)] |- \[[ xX]\] )/.test(line);
}

/**
 * 重建选中行：逐段拼接原始文本与变换后的行，
 * 并返回变换后选中块的 [start, end]（基于新 value 的索引）。
 */
function rebuildLines(value, ranges, nextLines) {
	let result = "";
	let cursor = 0;
	ranges.forEach((range, index) => {
		result += value.slice(cursor, range.lineStart) + nextLines[index];
		cursor = range.lineEnd;
	});
	result += value.slice(cursor);

	// 新选中块起点：首个变换行在原文的位置（其前的文本不变）。
	const newStart = ranges[0].lineStart;
	// 终点：起点 + 各变换行长度 + 行间换行数
	let newEnd = newStart;
	nextLines.forEach((line, index) => {
		newEnd += line.length;
		if (index < nextLines.length - 1) newEnd += 1; // 行间单个 \n
	});
	return { value: result, selectionStart: newStart, selectionEnd: newEnd };
}

/**
 * Line prefix transforms：quote / list / ordered-list / check-list。
 * 统一处理「覆盖到的完整逻辑行」。
 */
function applyLineTransform(input, command) {
	const { start, end } = normalizeSelection(input);
	const ranges = selectedLineRanges(input.value, start, end);
	if (!ranges.length)
		return { value: input.value, selectionStart: start, selectionEnd: end };

	const marker = {
		quote: "> ",
		list: "- ",
		"ordered-list": null, // 动态序号
		"check-list": "- [ ] ",
	}[command];
	const wantRemove = {
		quote: (line) => /^> /.test(line),
		list: (line) => /^- /.test(line) || /^\* /.test(line) || /^\+ /.test(line),
		"ordered-list": (line) => /^\d+[.)] /.test(line),
		"check-list": (line) => /^- \[[ xX]\] /.test(line),
	}[command];

	const lines = ranges.map((range) =>
		input.value.slice(range.lineStart, range.lineEnd),
	);
	const allMarked = lines.length > 0 && lines.every((line) => wantRemove(line));

	const nextLines = lines.map((line, index) => {
		if (!line.trim()) return line;
		if (allMarked) {
			return line.replace(LINE_PREFIX_RE, "");
		}
		let body = line.replace(LINE_PREFIX_RE, "");
		if (command === "ordered-list") {
			const number = index + 1;
			return `${number}. ${body}`;
		}
		return `${marker}${body}`;
	});

	return rebuildLines(input.value, ranges, nextLines);
}

const HEADING_LEVELS = new Set([1, 2, 3, 4, 5, 6]);

/**
 * Heading：处理整行。level 0 = 正文（移除行首 `#{1,6} `）；
 * 1–6 替换/设置对应 heading marker。多行选区对每个非空行处理。
 */
function applyHeadingCommand(input, level) {
	const { start, end } = normalizeSelection(input);
	const ranges = selectedLineRanges(input.value, start, end);
	const lines = ranges.map((range) =>
		input.value.slice(range.lineStart, range.lineEnd),
	);

	const nextLines = lines.map((line) => {
		if (!line.trim()) return line;
		const body = line.replace(/^\s{0,3}#{1,6}\s+/, "");
		if (!HEADING_LEVELS.has(level) || level === 0 || level === undefined) {
			return body;
		}
		return `${"#".repeat(level)} ${body}`;
	});

	return rebuildLines(input.value, ranges, nextLines);
}

/**
 * Table：插入稳定 2×2 模板。光标不在空行时前后补换行，避免黏连。
 * 插入后选中 `Column 1`。
 */
function applyTableCommand(input) {
	const { start, end } = normalizeSelection(input);
	const template = [
		"| Column 1 | Column 2 |",
		"| --- | --- |",
		"|  |  |",
		"|  |  |",
	].join("\n");

	const prefixWrap = input.value.slice(0, start).endsWith("\n") ||
		start === 0 ? "" : "\n";
	const suffixWrap = input.value.slice(end).startsWith("\n") ||
		end === input.value.length ? "" : "\n";

	const insert = `${prefixWrap}${template}${suffixWrap}`;
	const value =
		input.value.slice(0, start) + insert + input.value.slice(end);
	const colStart = start + prefixWrap.length + 2;

	return {
		value,
		selectionStart: colStart,
		selectionEnd: colStart + "Column 1".length,
	};
}

/**
 * Horizontal line：插入独占一行的 `---`，必要处补换行。
 */
function applyHrCommand(input) {
	const { start, end } = normalizeSelection(input);
	const before = input.value.slice(0, start);
	const after = input.value.slice(end);
	const beforeWrap = !before || before.endsWith("\n") ? "" : "\n";
	const afterWrap = !after || after.startsWith("\n") ? "" : "\n";
	const value = `${before}${beforeWrap}---${afterWrap}${after}`;
	return {
		value,
		selectionStart: start + beforeWrap.length,
		selectionEnd: start + beforeWrap.length + 3,
	};
}

/**
 * Upload image (workspace local)：插入 `![alt](markdownPath)`。
 * 由工具栏 upload 流程调用；不是纯文本命令但输出同一 return 契约。
 */
function applyUploadImageCommand(input, options) {
	const { start, end } = normalizeSelection(input);
	const alt = options.alt || "image";
	const markdownPath = options.markdownPath || "";
	const insert = `![${alt}](${markdownPath})`;
	return {
		value: `${input.value.slice(0, start)}${insert}${input.value.slice(end)}`,
		selectionStart: start + insert.length,
		selectionEnd: start + insert.length,
	};
}

function applyCommand(input) {
	const command = input.command;
	const options = input.options || {};
	const base = {
		value: input.value,
		selectionStart: input.selectionStart ?? 0,
		selectionEnd: input.selectionEnd ?? input.selectionStart ?? 0,
	};

	switch (command) {
		case "bold":
			return applyInlineToggle(base, "**", "**", "bold");
		case "italic":
			return applyInlineToggle(base, "*", "*", "italic");
		case "strikethrough":
			return applyInlineToggle(base, "~~", "~~", "strikethrough");
		case "code":
			return applyCodeCommand(base);
		case "link":
			return applyLinkCommand(base);
		case "image":
			return applyImageCommand(base);
		case "image-upload":
			return applyUploadImageCommand(base, options);
		case "quote":
		case "list":
		case "ordered-list":
		case "check-list":
			return applyLineTransform(base, command);
		case "heading":
			return applyHeadingCommand(base, Number(options.level));
		case "table":
			return applyTableCommand(base);
		case "hr":
			return applyHrCommand(base);
		default:
			return base;
	}
}

const api = {
	applyCommand,
	applyInlineToggle,
	applyCodeCommand,
	applyLinkCommand,
	applyImageCommand,
	applyUploadImageCommand,
	applyLineTransform,
	applyHeadingCommand,
	applyTableCommand,
	applyHrCommand,
	selectedLineRanges,
	lineBounds,
	INLINE_COMMANDS,
	LINE_COMMANDS,
};

if (typeof window !== "undefined") {
	window.StillwriteMarkdownFormatting = api;
}
if (typeof module !== "undefined" && module.exports) {
	module.exports = api;
}