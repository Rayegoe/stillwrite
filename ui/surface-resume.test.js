"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const SurfaceResume = require("./surface-resume.js");

function fakeStorage(initial = {}) {
	const map = new Map(Object.entries(initial));
	return {
		getItem: (key) => (map.has(key) ? map.get(key) : null),
		setItem: (key, value) => map.set(key, String(value)),
		removeItem: (key) => map.delete(key),
	};
}

test("read 接受合法 surface，损坏或未知类型返回 null", () => {
	const storage = fakeStorage({
		[SurfaceResume.STORAGE_KEY]: JSON.stringify({
			type: "workspace",
			uri: "/tmp/a.md",
		}),
	});
	assert.deepEqual(SurfaceResume.read(storage), {
		type: "workspace",
		uri: "/tmp/a.md",
	});
	const broken = fakeStorage({ [SurfaceResume.STORAGE_KEY]: "{not json" });
	assert.equal(SurfaceResume.read(broken), null);
	const unknown = fakeStorage({
		[SurfaceResume.STORAGE_KEY]: JSON.stringify({ type: "kanban" }),
	});
	assert.equal(SurfaceResume.read(unknown), null);
	const empty = fakeStorage();
	assert.equal(SurfaceResume.read(empty), null);
});

test("write 忽略未知类型，roundtrip 保留字段", () => {
	const storage = fakeStorage();
	SurfaceResume.write(storage, { type: "kanban" });
	assert.equal(storage.getItem(SurfaceResume.STORAGE_KEY), null);
	const surface = { type: "artifact", id: "aw-1", parentWorkId: "w-1" };
	SurfaceResume.write(storage, surface);
	assert.deepEqual(SurfaceResume.read(storage), surface);
});

test("planResume 按优先级：上次 Surface → 最近文档 → Work Home", () => {
	assert.deepEqual(
		SurfaceResume.planResume({
			surface: { type: "library", sourceId: "s1", relativePath: "a.md" },
			surfaceAvailable: true,
			fallbackDocument: "/tmp/recent.md",
		}),
		{ kind: "surface", surface: { type: "library", sourceId: "s1", relativePath: "a.md" } },
	);
	// 上次对象已不存在 → 最近可用 Workspace 文档
	assert.deepEqual(
		SurfaceResume.planResume({
			surface: { type: "artifact", id: "gone" },
			surfaceAvailable: false,
			fallbackDocument: "/tmp/recent.md",
		}),
		{ kind: "fallback-document", path: "/tmp/recent.md" },
	);
	// 无记录 → 最近可用文档仍然恢复
	assert.deepEqual(
		SurfaceResume.planResume({ surface: null, fallbackDocument: "/tmp/r.md" }),
		{ kind: "fallback-document", path: "/tmp/r.md" },
	);
	// 什么都没有 → Work Home / empty state
	assert.deepEqual(SurfaceResume.planResume({}), { kind: "work-home" });
});
