const test = require("node:test");
const assert = require("node:assert/strict");
const FileMenu = require("./file-menu.js");

test("menuState follows the File menu state matrix", () => {
	// no workspace：只保留 新建 / 打开 / 最近打开
	assert.deepEqual(
		FileMenu.menuState({
			hasWorkspace: false,
			currentFile: null,
			libraryDoc: null,
			agentDoc: null,
		}),
		{
			newFile: true,
			open: true,
			recent: true,
			save: false,
			saveAs: false,
			workspace: false,
		},
	);

	// Workspace doc：全开
	assert.deepEqual(
		FileMenu.menuState({
			hasWorkspace: true,
			currentFile: "/ws/notes.md",
			libraryDoc: null,
			agentDoc: null,
		}),
		{
			newFile: true,
			open: true,
			recent: true,
			save: true,
			saveAs: true,
			workspace: true,
		},
	);

	// Library doc：保存 / 另存为禁用
	assert.deepEqual(
		FileMenu.menuState({
			hasWorkspace: true,
			currentFile: null,
			libraryDoc: { sourceId: "s1" },
			agentDoc: null,
		}),
		{
			newFile: true,
			open: true,
			recent: true,
			save: false,
			saveAs: false,
			workspace: true,
		},
	);

	// Agent Work：可保存，不可另存为
	assert.deepEqual(
		FileMenu.menuState({
			hasWorkspace: true,
			currentFile: null,
			libraryDoc: null,
			agentDoc: { id: "aw1" },
		}),
		{
			newFile: true,
			open: true,
			recent: true,
			save: true,
			saveAs: false,
			workspace: true,
		},
	);

	// Work surface：无文档，工作区子菜单仍可用
	assert.deepEqual(
		FileMenu.menuState({
			hasWorkspace: true,
			currentFile: null,
			libraryDoc: null,
			agentDoc: null,
		}),
		{
			newFile: true,
			open: true,
			recent: true,
			save: false,
			saveAs: false,
			workspace: true,
		},
	);
});

test("menuState guards against stale workspace doc when library doc is present", () => {
	// Library 打开时不允许残留的 currentFile 把保存/另存为点亮
	assert.equal(
		FileMenu.menuState({
			hasWorkspace: true,
			currentFile: "/ws/old.md",
			libraryDoc: { sourceId: "s1" },
			agentDoc: null,
		}).save,
		false,
	);
	assert.equal(
		FileMenu.menuState({
			hasWorkspace: true,
			currentFile: "/ws/old.md",
			libraryDoc: { sourceId: "s1" },
			agentDoc: null,
		}).saveAs,
		false,
	);
});

test("recentItemModel projects workspace and document entries", () => {
	const workspace = FileMenu.recentItemModel({
		id: 1,
		kind: "workspace",
		path: "/home/me/stillwrite",
		label: "stillwrite",
		available: true,
		openedAt: "2026-09-01T10:00:00Z",
	});
	assert.equal(workspace.icon, "▣");
	assert.equal(workspace.displayLabel, "stillwrite");
	assert.equal(workspace.disabled, false);
	assert.equal(workspace.title, "/home/me/stillwrite");

	const missing = FileMenu.recentItemModel({
		id: 2,
		kind: "document",
		path: "/mnt/gone/notes.md",
		label: "notes.md",
		available: false,
		openedAt: "2026-09-01T09:00:00Z",
	});
	assert.equal(missing.icon, "▤");
	assert.equal(missing.disabled, true);
	assert.equal(missing.displayLabel, "notes.md（不可用）");
	assert.equal(missing.title, "/mnt/gone/notes.md");
});

test("recentItemModel keeps unknown kinds out of the workspace category", () => {
	const item = FileMenu.recentItemModel({
		id: 3,
		kind: "bogus",
		path: "/x/y.md",
		label: "y.md",
		available: true,
		openedAt: "2026-09-01T08:00:00Z",
	});
	assert.equal(item.kind, "document");
	assert.equal(item.icon, "▤");
});

test("nextEnabledMenuItem skips disabled entries and wraps", () => {
	const items = [
		{ id: "a" },
		{ id: "b", disabled: true },
		{ id: "c" },
		{ id: "d", disabled: true },
	];
	assert.equal(FileMenu.nextEnabledMenuItem(items, 0, 1), 2);
	assert.equal(FileMenu.nextEnabledMenuItem(items, 2, 1), 0);
	assert.equal(FileMenu.nextEnabledMenuItem(items, 0, -1), 2);
	assert.equal(FileMenu.nextEnabledMenuItem(items, 2, -1), 0);
	assert.equal(FileMenu.nextEnabledMenuItem([], -1, 1), -1);
});

test("nextEnabledMenuItem treats hidden entries as unreachable", () => {
	const items = [{ id: "a", hidden: true }, { id: "b" }];
	assert.equal(FileMenu.nextEnabledMenuItem(items, 1, 1), 1);
	assert.equal(FileMenu.edgeEnabledMenuItem(items, "first"), 1);
});

test("edgeEnabledMenuItem finds first and last enabled items", () => {
	const items = [
		{ id: "a", disabled: true },
		{ id: "b" },
		{ id: "c", disabled: true },
		{ id: "d" },
	];
	assert.equal(FileMenu.edgeEnabledMenuItem(items, "first"), 1);
	assert.equal(FileMenu.edgeEnabledMenuItem(items, "end"), 3);
	assert.equal(FileMenu.edgeEnabledMenuItem([{ id: "x", disabled: true }], "first"), -1);
});
