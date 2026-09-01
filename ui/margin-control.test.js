const test = require("node:test");
const assert = require("node:assert/strict");
const MarginControl = require("./margin-control.js");

const KEY = MarginControl.MARGIN_STORAGE_KEY;

// 假 localStorage：该控件只使用 getItem / setItem / removeItem。
function createFakeStorage(seed = {}) {
	const store = new Map(Object.entries(seed));
	return {
		getItem(key) {
			return store.has(key) ? store.get(key) : null;
		},
		setItem(key, value) {
			store.set(key, String(value));
		},
		removeItem(key) {
			store.delete(key);
		},
		entries() {
			return [...store.entries()];
		},
	};
}

// 记录型 DOM 写入回调：每次投影完整记下 label / CSS 变量 / body class 变更。
function createRecorder() {
	const projections = [];
	return {
		projections,
		apply(projection) {
			projections.push({
				value: projection.value,
				label: projection.label,
				setCss: [...projection.setCss],
				removeCss: [...projection.removeCss],
				addClass: [...projection.addClass],
				removeClass: [...projection.removeClass],
			});
		},
	};
}

function last(recorder) {
	return recorder.projections.at(-1);
}

test("未设置 stillwrite.markdownPaddingX → auto：显示 AUTO，不加 body.margin-fixed，不写 CSS 变量", () => {
	const storage = createFakeStorage();
	const recorder = createRecorder();
	const state = MarginControl.create({ storage, apply: recorder.apply });
	state.sync();

	assert.equal(state.value, null);
	const projection = last(recorder);
	assert.equal(projection.label, "AUTO");
	assert.ok(
		!projection.addClass.includes("margin-fixed"),
		"auto 不得添加 margin-fixed",
	);
	assert.deepEqual(projection.setCss, [], "auto 不写固定边距变量");
	assert.deepEqual(projection.removeCss, [
		"--markdown-padding-x",
		"--editor-padding-x",
	]);
	assert.deepEqual(projection.removeClass, ["margin-fixed"]);
});

test("已设置 localStorage 固定值 → 原值保留：显示 px，加 body.margin-fixed", () => {
	const storage = createFakeStorage({ [KEY]: "64" });
	const recorder = createRecorder();
	const state = MarginControl.create({ storage, apply: recorder.apply });
	state.sync();

	assert.equal(state.value, 64);
	const projection = last(recorder);
	assert.equal(projection.label, "64");
	assert.deepEqual(projection.addClass, ["margin-fixed"]);
	assert.deepEqual(projection.setCss, [
		["--markdown-padding-x", "64px"],
		["--editor-padding-x", "64px"],
	]);
});

test("自动 → 固定：auto 状态按 − / ＋ 以 seed 起步进入 fixed，然后正常步进", () => {
	const storage = createFakeStorage();
	const recorder = createRecorder();
	const state = MarginControl.create({ storage, apply: recorder.apply });

	// auto 起步：+ → 72 + 8 = 80，− → 72 − 8 = 64
	assert.equal(state.step(+1), 80);
	assert.equal(state.value, 80);
	assert.equal(storage.getItem(KEY), "80");
	assert.deepEqual(last(recorder).addClass, ["margin-fixed"]);
	assert.equal(last(recorder).label, "80");

	assert.equal(state.step(-1), 72);
	assert.equal(last(recorder).label, "72");

	assert.equal(state.step(-1), 64);
	assert.equal(last(recorder).label, "64");
	assert.equal(storage.getItem(KEY), "64");
});

test("固定 → 自动：点击数值删除存储键，立即恢复 AUTO 且不加 body.margin-fixed", () => {
	const storage = createFakeStorage({ [KEY]: "64" });
	const recorder = createRecorder();
	const state = MarginControl.create({ storage, apply: recorder.apply });
	state.sync();
	assert.equal(state.value, 64);

	const returned = state.resetToAuto();
	assert.equal(returned, null);
	assert.equal(state.value, null);
	assert.equal(storage.getItem(KEY), null, "存储键必须被删除");
	assert.deepEqual(storage.entries(), [], "不得遗留其他持久化结构");

	const projection = last(recorder);
	assert.equal(projection.label, "AUTO");
	assert.ok(
		!projection.addClass.includes("margin-fixed"),
		"auto 不得添加 margin-fixed",
	);
	assert.deepEqual(projection.removeClass, ["margin-fixed"]);
	assert.deepEqual(projection.setCss, [], "auto 不写固定边距变量");
});

test("完整往返 auto → fixed → auto：每一步 DOM 投影与存储一致", () => {
	const storage = createFakeStorage();
	const recorder = createRecorder();
	const state = MarginControl.create({ storage, apply: recorder.apply });
	state.sync();

	// auto
	assert.equal(last(recorder).label, "AUTO");
	assert.ok(!last(recorder).addClass.includes("margin-fixed"));

	// 进入 fixed（seed 步进）
	state.step(+1);
	assert.equal(
		state.value,
		MarginControl.MARGIN_AUTO_SEED + MarginControl.MARGIN_STEP,
	);
	assert.equal(last(recorder).label, String(state.value));
	assert.deepEqual(last(recorder).addClass, ["margin-fixed"]);
	assert.equal(storage.getItem(KEY), String(state.value));

	// 再步进
	state.step(+1);
	assert.equal(state.value, 88);

	// 回到 auto
	state.resetToAuto();
	assert.equal(state.value, null);
	assert.equal(last(recorder).label, "AUTO");
	assert.ok(!last(recorder).addClass.includes("margin-fixed"));
	assert.equal(storage.getItem(KEY), null);

	// auto 状态下再按 ＋ 重新进入 fixed
	state.step(+1);
	assert.equal(state.value, 80);
	assert.deepEqual(last(recorder).addClass, ["margin-fixed"]);
	assert.equal(storage.getItem(KEY), "80");
});

test("步进限制在 [16, 480]，越界被 clamp", () => {
	const storage = createFakeStorage();
	const recorder = createRecorder();
	const state = MarginControl.create({ storage, apply: recorder.apply });
	state.sync();

	// 从 auto seed 72 起步：72 + 8×60 = 552 → clamp 到 480
	for (let i = 0; i < 60; i++) state.step(+1);
	assert.equal(state.value, MarginControl.MARGIN_MAX);

	// 480 − 8×70 = −80 → clamp 到 16
	for (let i = 0; i < 70; i++) state.step(-1);
	assert.equal(state.value, MarginControl.MARGIN_MIN);
	assert.equal(storage.getItem(KEY), String(MarginControl.MARGIN_MIN));
});

test("存储键非法数值 → 按 auto 处理，不加 body.margin-fixed", () => {
	const storage = createFakeStorage({ [KEY]: "not-a-number" });
	const recorder = createRecorder();
	const state = MarginControl.create({ storage, apply: recorder.apply });
	state.sync();

	assert.equal(state.value, null);
	assert.equal(last(recorder).label, "AUTO");
	assert.ok(!last(recorder).addClass.includes("margin-fixed"));
});

test("displayLabel 与 steppedValue 纯函数行为", () => {
	assert.equal(MarginControl.displayLabel(null), "AUTO");
	assert.equal(MarginControl.displayLabel(96), "96");
	assert.equal(
		MarginControl.steppedValue(null, -1),
		MarginControl.MARGIN_AUTO_SEED - MarginControl.MARGIN_STEP,
	);
	assert.equal(
		MarginControl.steppedValue(null, +1),
		MarginControl.MARGIN_AUTO_SEED + MarginControl.MARGIN_STEP,
	);
	assert.equal(MarginControl.steppedValue(100, -1), 92);
	assert.equal(MarginControl.steppedValue(100, +1), 108);
});
