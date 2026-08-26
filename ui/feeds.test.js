const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

// feeds.js 挂到 window/globalThis；在 Node 里执行一遍拿到命名空间。
const window = {};
const code = fs.readFileSync(path.join(__dirname, "feeds.js"), "utf8");
new Function("window", "globalThis", code)(window, window);
const Feeds = window.StillwriteFeeds;

test("feedSourceStatusText 优先展示错误", () => {
	assert.equal(
		Feeds.feedSourceStatusText({ last_error: "HTTP 403" }),
		"失败：HTTP 403",
	);
});

test("feedSourceStatusText 展示成功时间", () => {
	const text = Feeds.feedSourceStatusText({ last_fetch_at: 1787692800 });
	assert.match(text, /^成功 · /);
});

test("feedSourceStatusText 从未抓取时提示", () => {
	assert.equal(Feeds.feedSourceStatusText({}), "尚未抓取");
});

test("rssSourceCountText 无源时提示", () => {
	assert.equal(Feeds.rssSourceCountText([], null), "还没有源");
});

test("rssSourceCountText 组合源数与篇数", () => {
	const sources = [{ id: "a" }, { id: "b" }];
	assert.equal(
		Feeds.rssSourceCountText(sources, { documents: 128 }),
		"2 源 · 128 篇",
	);
});
