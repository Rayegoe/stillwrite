const test = require("node:test");
const assert = require("node:assert/strict");
const annotations = require("./annotations.js");

test("旧版整篇批注兼容为全文批注", () => {
	const items = annotations.parse("一段旧批注\n\n还有一段", "2026-08-25 09:30");
	assert.equal(items.length, 1);
	assert.equal(items[0].kind, "document");
	assert.equal(items[0].note, "一段旧批注\n\n还有一段");
	assert.equal(items[0].updatedAt, "2026-08-25 09:30");
});

test("结构化批注可读写往返且保留中文和多行原文", () => {
	const source = [
		{
			id: "note-1",
			kind: "paragraph",
			start: 4,
			end: 16,
			quote: "第一行原文\n第二行原文",
			note: "观点很清楚。\n\n但还可补一个例子。",
			createdAt: "2026-08-25 10:00",
			updatedAt: "2026-08-25 10:02",
		},
	];
	const markdown = annotations.serialize(source);
	assert.match(markdown, /原文（段落）/);
	assert.deepEqual(annotations.parse(markdown), source);
});

test("无选区时捕获光标所在段落", () => {
	const source = "第一段。\n\n第二段第一行，\n第二段第二行。\n\n第三段。";
	const cursor = source.indexOf("第二段第二行");
	const range = annotations.selectionOrParagraph(source, cursor, cursor);
	assert.equal(range.kind, "paragraph");
	assert.equal(range.quote, "第二段第一行，\n第二段第二行。");
});

test("原文位置变化后按最近的相同引文重新锚定", () => {
	const item = { quote: "重复句", start: 4, end: 7 };
	const range = annotations.resolveRange("前缀重复句。更长前缀重复句", item);
	assert.equal(range.start, 2);
	assert.equal(range.end, 5);
});
