const assert = require("assert");
const Annotations = require("./annotations.js");
const Links = require("./library-annotation-links.js");

const source = {
	uri: "library://source-1/notes/a.md",
	sourceId: "source-1",
	relativePath: "notes/a.md",
	title: "A",
	sourceName: "资料库",
};
const own = {
	id: "own-1",
	kind: "selection",
	start: 1,
	end: 2,
	quote: "作品原文",
	note: "自己的批注",
	createdAt: "2026-08-25 10:00",
	updatedAt: "2026-08-25 10:00",
};
const first = Links.syncLinkedItems([own], source, [
	{
		id: "lib-1",
		kind: "selection",
		quote: "资料原文",
		note: "过去的思考",
		createdAt: "2026-08-25 11:00",
		updatedAt: "2026-08-25 11:00",
	},
]);
assert.equal(first.length, 2);
assert.equal(first[0].id, "own-1");
const parsed = Links.parseLinkedNote(first[1].note);
assert(parsed);
assert.equal(parsed.meta.uri, source.uri);
assert.equal(parsed.meta.originId, "lib-1");
assert.equal(parsed.note, "过去的思考");
assert.equal(first[1].quote, "资料原文");

const second = Links.syncLinkedItems(first, source, [
	{
		id: "lib-1",
		quote: "资料原文（更新）",
		note: "过去的思考（更新）",
		updatedAt: "2026-08-25 12:00",
	},
]);
assert.equal(second.length, 2, "同一资料重新同步应替换镜像而不是重复追加");
const updated = Links.parseLinkedNote(second[1].note);
assert.equal(updated.note, "过去的思考（更新）");
assert.equal(second[1].quote, "资料原文（更新）");

const deleted = Links.syncLinkedItems(second, source, []);
assert.deepEqual(deleted.map((item) => item.id), ["own-1"], "删除资料批注时应删除作品侧镜像");

const otherSource = {
	uri: "library://source-2/b.md",
	sourceId: "source-2",
	relativePath: "b.md",
	title: "B",
};
const withOther = Links.syncLinkedItems(first, otherSource, [
	{ id: "lib-2", quote: "B 原文", note: "B 批注" },
]);
assert.equal(withOther.length, 3, "不同资料的镜像不能互相覆盖");

// 回归：三条资料批注经过完整的 sidecar 序列化/回读后，
// 每一条都必须仍能解析出资料标题，而不能把 base64 标记泄漏到 UI。
const three = Links.syncLinkedItems([], source, [
	{ id: "lib-1", quote: "资料原文一", note: "批注一" },
	{ id: "lib-2", quote: "资料原文二", note: "批注二" },
	{ id: "lib-3", quote: "资料原文三", note: "批注三" },
]);
const reloaded = Annotations.parse(Annotations.serialize(three));
assert.equal(reloaded.length, 3, "三条资料批注回读后都应保留");
assert.equal(new Set(reloaded.map((item) => item.id)).size, 3);
assert.deepEqual(
	reloaded.map((item) => Links.parseLinkedNote(item.note)?.meta.title),
	["A", "A", "A"],
);
assert.deepEqual(
	reloaded.map((item) => Links.parseLinkedNote(item.note)?.note),
	["批注一", "批注二", "批注三"],
);

// 容忍旧侧车可能带有 BOM/缩进，避免编码标记直接显示在批注框中。
const padded = `\uFEFF  ${three[0].note}`;
assert.equal(Links.parseLinkedNote(padded)?.meta.title, "A");
assert.equal(Links.parseLinkedNote(three[1].note.replaceAll("\n", "\r\n"))?.meta.title, "A");

console.log("library annotation links tests passed");
