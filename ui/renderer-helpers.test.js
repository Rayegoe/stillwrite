const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

// 从 app.js 逐字提取纯 renderer helper 并测试。
// app.js 是 DOM 耦合的浏览器脚本，不能 require；提取函数体是唯一不重构它的测试入口。
const appSource = fs.readFileSync(path.join(__dirname, "app.js"), "utf8");

function extractFunc(name) {
	const re = new RegExp(`^function ${name}\\(`, "m");
	const start = appSource.search(re);
	if (start < 0) throw new Error(`function ${name} not found`);
	const bodyMatch = appSource.slice(start).search(/\)\s*\{/);
	if (bodyMatch < 0) throw new Error(`no body brace for ${name}`);
	const open =
		start + bodyMatch + appSource.slice(start + bodyMatch).indexOf("{");
	let depth = 0;
	let i = open;
	for (; i < appSource.length; i += 1) {
		if (appSource[i] === "{") depth += 1;
		else if (appSource[i] === "}") {
			depth -= 1;
			if (depth === 0) break;
		}
	}
	return appSource.slice(start, i + 1);
}

const helpers = [
	"splitTableRow",
	"isTableDelimiter",
	"tableCellAlignClass",
	"escapeHtml",
	"renderInline",
];
const source = helpers.map(extractFunc).join("\n");
const sandbox = { console, Math, NodeFilter: { SHOW_TEXT: 4 } };
const factory = new Function(
	...Object.keys(sandbox),
	`"use strict";\n${source}\nreturn { splitTableRow, isTableDelimiter, tableCellAlignClass, escapeHtml, renderInline };`,
);
const api = factory(...Object.values(sandbox));

/* ---------- splitTableRow ---------- */

test("splitTableRow: 首尾 pipe 可省略", () => {
	assert.deepEqual(api.splitTableRow("| A | B |"), ["A", "B"]);
	assert.deepEqual(api.splitTableRow("A | B"), ["A", "B"]);
});

test("splitTableRow: \\| 是正文 pipe，不拆列", () => {
	const cells = api.splitTableRow("a \\| b | c");
	assert.equal(cells.length, 2);
	assert.ok(cells[0].includes("|"), `第一格保留 pipe: ${cells[0]}`);
	assert.equal(cells[1], "c");
});

test("splitTableRow: 空格裁剪", () => {
	assert.deepEqual(api.splitTableRow("  a   |   b  "), ["a", "b"]);
});

/* ---------- isTableDelimiter ---------- */

test("isTableDelimiter: 合法分隔行", () => {
	assert.equal(api.isTableDelimiter("| --- | --- |"), true);
	assert.equal(api.isTableDelimiter(":--- | :---: | ---:"), true);
	assert.equal(api.isTableDelimiter("---|---"), true);
});

test("isTableDelimiter: 普通行不算分隔行", () => {
	assert.equal(api.isTableDelimiter("a | b"), false);
	assert.equal(api.isTableDelimiter("| a |"), false);
	assert.equal(api.isTableDelimiter("--- | --- | x"), false);
	assert.equal(api.isTableDelimiter("| a"), false);
});

/* ---------- tableCellAlignClass ---------- */

test("tableCellAlignClass: 对齐类", () => {
	assert.equal(api.tableCellAlignClass(":---"), "align-left");
	assert.equal(api.tableCellAlignClass(":---:"), "align-center");
	assert.equal(api.tableCellAlignClass("---:"), "align-right");
	assert.equal(api.tableCellAlignClass("---"), "align-left");
});

/* ---------- escapeHtml ---------- */

test("escapeHtml: 保留原有语义", () => {
	assert.equal(api.escapeHtml("<a b=\"1\">"), "&lt;a b=&quot;1&quot;&gt;");
	assert.equal(api.escapeHtml("A & B"), "A &amp; B");
});

/* ---------- renderInline：图片安全 ---------- */

test("renderInline: 远程图片直接 src", () => {
	const out = api.renderInline("![alt](https://ex.com/a.png)");
	assert.ok(out.includes('<img src="https://ex.com/a.png"'), out);
});

test("renderInline: 本地相对图片走 data-local-src", () => {
	const out = api.renderInline("![diagram](assets/diagram.png)");
	assert.ok(out.includes('data-local-src="assets/diagram.png"'), out);
	// 只允许 data-local-src 属性名，不能出现 src= 直接加载本地路径
	assert.ok(!out.includes('<img src="assets'), out);
});

test("renderInline: javascript:/data: 图片不进 src", () => {
	const out = api.renderInline("![x](javascript:alert(1))");
	assert.ok(out.includes("data-local-src"), out);
	const dataOut = api.renderInline("![x](data:image/png;base64,xxx)");
	assert.ok(dataOut.includes("data-local-src"), dataOut);
});

test("renderInline: 原有格式回归", () => {
	assert.ok(api.renderInline("**bold**").includes("<strong>bold</strong>"));
	assert.ok(api.renderInline("*em*").includes("<em>em</em>"));
	assert.ok(api.renderInline("~~del~~").includes("<del>del</del>"));
	assert.ok(api.renderInline("[link](https://a.b)").includes("<a href=\"https://a.b\""));
	assert.ok(api.renderInline("`code`").includes("<code>code</code>"));
});