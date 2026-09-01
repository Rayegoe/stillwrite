const test = require("node:test");
const assert = require("node:assert/strict");
const md = require("./markdown-formatting.js");

function run(value, selectionStart, selectionEnd, command, options) {
	return md.applyCommand({ value, selectionStart, selectionEnd, command, options });
}

function assertSelection(result, message) {
	assert.ok(
		Number.isInteger(result.selectionStart) && result.selectionStart >= 0,
		`${message}: selectionStart=${result.selectionStart}`,
	);
	assert.ok(
		Number.isInteger(result.selectionEnd) && result.selectionEnd >= 0,
		`${message}: selectionEnd=${result.selectionEnd}`,
	);
	assert.ok(
		result.selectionStart <= result.value.length &&
			result.selectionEnd <= result.value.length,
		`${message}: selection超出 value.length`,
	);
	assert.ok(result.selectionStart <= result.selectionEnd, `${message}: start>end`);
}

/* ---------- A. Text transform matrix ---------- */

test("bold: abc → **abc**", () => {
	const r = run("hello world", 0, 5, "bold");
	assert.equal(r.value, "**hello** world");
	assert.equal(r.value.slice(r.selectionStart, r.selectionEnd), "hello");
});

test("bold toggle: **abc** → abc", () => {
	const r = run("say **abc** now", 4, 11, "bold");
	assert.equal(r.value, "say abc now");
	assert.equal(r.value.slice(r.selectionStart, r.selectionEnd), "abc");
});

test("bold 无选区插入 placeholder", () => {
	const r = run("", 0, 0, "bold");
	assert.equal(r.value, "**bold**");
	assert.equal(r.value.slice(r.selectionStart, r.selectionEnd), "bold");
});

test("italic: hello → *hello*；toggle 移除", () => {
	const r1 = run("hello", 0, 5, "italic");
	assert.equal(r1.value, "*hello*");
	const r2 = run(r1.value, r1.selectionStart, r1.selectionEnd, "italic");
	assert.equal(r2.value, "hello");
});

test("strikethrough: hello → ~~hello~~；toggle 移除", () => {
	const r1 = run("hello", 0, 5, "strikethrough");
	assert.equal(r1.value, "~~hello~~");
	const r2 = run(r1.value, r1.selectionStart, r1.selectionEnd, "strikethrough");
	assert.equal(r2.value, "hello");
});

test("inline code: value → `value`", () => {
	const r = run("value", 0, 5, "code");
	assert.equal(r.value, "`value`");
});

test("inline code 内含反引号升级双反引号", () => {
	const r = run("a ` b", 0, 5, "code");
	assert.equal(r.value, "``a ` b``");
});

test("code 多行 → fenced block；再点移除", () => {
	const r1 = run("line1\nline2", 0, 11, "code");
	assert.equal(r1.value, "```\nline1\nline2\n```");
	const r2 = run(r1.value, r1.selectionStart, r1.selectionEnd, "code");
	assert.equal(r2.value, "line1\nline2");
});

test("quote: 两行 → 每行 > 前缀；toggle 移除", () => {
	const r1 = run("a\nb", 0, 3, "quote");
	assert.equal(r1.value, "> a\n> b");
	const r2 = run(r1.value, r1.selectionStart, r1.selectionEnd, "quote");
	assert.equal(r2.value, "a\nb");
});

test("quote 混合前缀行只去掉已有 marker", () => {
	const r = run("> a\nb", 0, 5, "quote");
	assert.equal(r.value, "> a\n> b");
});

test("bullet: 两行 → - 前缀；再次点击移除", () => {
	const r1 = run("a\nb", 0, 3, "list");
	assert.equal(r1.value, "- a\n- b");
	const r2 = run(r1.value, r1.selectionStart, r1.selectionEnd, "list");
	assert.equal(r2.value, "a\nb");
});

test("list 已有 1. / * / - [x] marker 时替换不叠加", () => {
	const r1 = run("1. a\nb", 0, 6, "list");
	assert.equal(r1.value, "- a\n- b");
	const r2 = run("* a\nb", 0, 5, "list");
	assert.equal(r2.value, "- a\n- b");
	const r3 = run("- [x] a\nb", 0, 9, "list");
	assert.equal(r3.value, "- a\n- b");
});

test("ordered: 两行 → 1. / 2.；已有 marker 替换；再次点击移除", () => {
	const r1 = run("a\nb", 0, 3, "ordered-list");
	assert.equal(r1.value, "1. a\n2. b");
	const r2 = run("- a\nb", 0, 5, "ordered-list");
	assert.equal(r2.value, "1. a\n2. b");
	const r3 = run(r1.value, r1.selectionStart, r1.selectionEnd, "ordered-list");
	assert.equal(r3.value, "a\nb");
});

test("task: 两行 → - [ ] 前缀；- [x] 视为同一 task marker", () => {
	const r1 = run("a\nb", 0, 3, "check-list");
	assert.equal(r1.value, "- [ ] a\n- [ ] b");
	const r2 = run("- [x] a\nb", 0, 9, "check-list");
	assert.equal(r2.value, "- [ ] a\n- [ ] b");
	const r3 = run(r1.value, r1.selectionStart, r1.selectionEnd, "check-list");
	assert.equal(r3.value, "a\nb");
});

test("heading H2: Title → ## Title；#### Title → ## Title；## Title → Title", () => {
	const r1 = run("Title", 0, 5, "heading", { level: 2 });
	assert.equal(r1.value, "## Title");
	const r2 = run("#### Title", 0, 10, "heading", { level: 2 });
	assert.equal(r2.value, "## Title");
	const r3 = run(r2.value, 0, r2.value.length, "heading", { level: 0 });
	assert.equal(r3.value, "Title");
});

test("heading 多行选区每行分别处理", () => {
	const r = run("a\nb", 0, 3, "heading", { level: 1 });
	assert.equal(r.value, "# a\n# b");
});

test("link: 选区 OpenAI → [OpenAI](https://) 并选中 URL", () => {
	const r = run("OpenAI", 0, 6, "link");
	assert.equal(r.value, "[OpenAI](https://)");
	assert.equal(r.value.slice(r.selectionStart, r.selectionEnd), "https://");
});

test("link 无选区 → [link text](https://) 选中 link text", () => {
	const r = run("", 0, 0, "link");
	assert.equal(r.value, "[link text](https://)");
	assert.equal(r.value.slice(r.selectionStart, r.selectionEnd), "link text");
});

test("link 已完整选区保持不变", () => {
	const text = "see [OpenAI](https://openai.com) ok";
	const s = text.indexOf("[OpenAI]");
	const e = s + "[OpenAI](https://openai.com)".length;
	const r = run(text, s, e, "link");
	assert.equal(r.value, text);
});

test("image: 选区 diagram → ![diagram](https://) 选中 URL", () => {
	const r = run("diagram", 0, 7, "image");
	assert.equal(r.value, "![diagram](https://)");
	assert.equal(r.value.slice(r.selectionStart, r.selectionEnd), "https://");
});

test("image 无选区 → ![image](https://)", () => {
	const r = run("", 0, 0, "image");
	assert.equal(r.value, "![image](https://)");
});

test("table: 光标处插入 2×2 模板并选中 Column 1，段落中自动补换行", () => {
	const r = run("para", 4, 4, "table");
	assert.equal(
		r.value,
		"para\n| Column 1 | Column 2 |\n| --- | --- |\n|  |  |\n|  |  |",
	);
	assert.equal(r.value.slice(r.selectionStart, r.selectionEnd), "Column 1");
	const r2 = run("", 0, 0, "table");
	assert.equal(r2.value.startsWith("| Column 1 | Column 2 |"), true);
});

test("hr: 段落边界插入独占一行 ---", () => {
	const r = run("before\nafter", 6, 6, "hr");
	assert.equal(r.value, "before\n---\nafter");
});

/* ---------- B. Selection correctness ---------- */

test("中文选区按字符不按字节", () => {
	const r = run("写一段中文", 0, 5, "bold");
	assert.equal(r.value, "**写一段中文**");
	assert.equal(r.value.slice(r.selectionStart, r.selectionEnd), "写一段中文");
	assertSelection(r, "中文 bold");
});

test("emoji/surrogate pair 不异常", () => {
	const r = run("笔记📝整理", 2, 6, "italic");
	assert.equal(r.value, "笔记*📝整理*");
	assertSelection(r, "emoji italic");
});

test("placeholder 全部选中", () => {
	const cases = [
		["bold", "**bold**", "bold"],
		["italic", "*italic*", "italic"],
		["strikethrough", "~~strikethrough~~", "strikethrough"],
		["code", "`code`", "code"],
	];
	for (const [command, expectedValue, placeholder] of cases) {
		const r = run("", 0, 0, command);
		assert.equal(r.value, expectedValue, command);
		assert.equal(
			r.value.slice(r.selectionStart, r.selectionEnd),
			placeholder,
			`${command} placeholder`,
		);
		assertSelection(r, command);
	}
});

test("光标前后正文不丢失", () => {
	const r = run("头部 中部 尾部", 3, 5, "bold");
	assert.equal(r.value, "头部 **中部** 尾部");
	assertSelection(r, "上下文保持");
});

test("line transforms 选区 indices 保持有效", () => {
	const r1 = run("第一行\n第二行\n第三行", 0, 7, "quote");
	assertSelection(r1, "quote 多行");
	const r2 = run("第一行\n第二行\n第三行", 0, 7, "check-list");
	assertSelection(r2, "task 多行");
	const r3 = run("第一行\n第二行\n第三行", 0, 7, "ordered-list");
	assertSelection(r3, "ordered 多行");
});

test("CRLF 输入不破坏未选中正文", () => {
	const r = run("a\r\nb\r\nc", 0, 4, "quote");
	assert.equal(r.value, "> a\r\n> b\r\nc");
	assertSelection(r, "CRLF");
});

test("空行不参与 line transform", () => {
	const r = run("a\n\nb", 0, 4, "quote");
	assert.equal(r.value, "> a\n\n> b");
});

test("table 模板选中 Column 1 且 indices 有效", () => {
	const r = run("", 0, 0, "table");
	assertSelection(r, "table");
});

test("image-upload: 插入本地相对路径并选中尾部", () => {
	const r = run("图：", 3, 3, "image-upload", {
		alt: "diagram",
		markdownPath: "assets/diagram.png",
	});
	assert.equal(r.value, "图：![diagram](assets/diagram.png)");
	assert.equal(r.selectionStart, r.value.length);
	assertSelection(r, "image-upload");
});