const test = require("node:test");
const assert = require("node:assert/strict");
const links = require("./document-links.js");

const root = "/workspace";
const nodes = [
	{ name: "张三.md", path: "/workspace/人物/张三.md", is_dir: false },
	{ name: "索引.md", path: "/workspace/索引.md", is_dir: false },
	{
		name: "docs",
		path: "/workspace/docs",
		is_dir: true,
		children: [{ name: "需求 文档.md", path: "/workspace/docs/需求 文档.md", is_dir: false }],
	},
];
const index = links.buildIndex(nodes, root);
const absolutePath = "/workspace/docs/需求 文档.md";

test("项目内唯一文件名词干与普通 URL 都能识别", () => {
	const parts = links.segmentText("张三参考 https://example.com/a，然后查看索引.md。", index);
	assert.deepEqual(
		parts.filter((part) => part.type).map(({ label, type }) => [label, type]),
		[
			["张三", "internal"],
			["https://example.com/a", "external"],
			["索引.md", "internal"],
		],
	);
});

test("相对链接按当前文档目录及工作区根目录解析", () => {
	assert.equal(
		links.resolveInternalHref("<../人物/张三.md>", "/workspace/docs/当前.md", root, index)?.path,
		"/workspace/人物/张三.md",
	);
	assert.equal(
		links.resolveInternalHref("docs/%E9%9C%80%E6%B1%82%20%E6%96%87%E6%A1%A3.md", "/workspace/索引.md", root, index)?.path,
		"/workspace/docs/需求 文档.md",
	);
});

test("完整路径、file URL 和 Windows 分隔符都能解析到工作区文档", () => {
	assert.equal(
		links.resolveInternalHref(absolutePath, "/workspace/索引.md", root, index)?.path,
		absolutePath,
	);
	assert.equal(
		links.resolveInternalHref(
			"file:///workspace/docs/%E9%9C%80%E6%B1%82%20%E6%96%87%E6%A1%A3.md",
			"/workspace/索引.md",
			root,
			index,
		)?.path,
		absolutePath,
	);
	assert.equal(
	links.resolveInternalHref(
			"docs\\需求 文档.md",
			"/workspace/索引.md",
			root,
			index,
		)?.path,
		absolutePath,
	);
});

test("正文中的完整路径会被识别为内部文档链接", () => {
	const parts = links.segmentText(
		"请打开 /workspace/docs/需求 文档.md 或 file:///workspace/docs/%E9%9C%80%E6%B1%82%20%E6%96%87%E6%A1%A3.md 查看。",
		index,
	);
	assert.deepEqual(
		parts.filter((part) => part.type).map(({ label, href, type }) => [label, href, type]),
		[
			["/workspace/docs/需求 文档.md", absolutePath, "internal"],
			[
				"file:///workspace/docs/%E9%9C%80%E6%B1%82%20%E6%96%87%E6%A1%A3.md",
				absolutePath,
				"internal",
			],
		],
	);
});
