const test = require("node:test");
const assert = require("node:assert/strict");
const WorkView = require("./work-view.js");

test("status badges carry text labels for every known status and degrade neutrally", () => {
	assert.deepEqual(WorkView.statusBadge("needs_human"), {
		label: "需要你",
		tone: "orange",
	});
	for (const status of [
		"queued",
		"running",
		"needs_human",
		"blocked",
		"completed",
		"failed",
		"cancelled",
	]) {
		const badge = WorkView.statusBadge(status);
		assert.ok(badge.label.length > 0, `${status} 必须有文字`);
		assert.ok(badge.tone.length > 0);
	}
	assert.deepEqual(WorkView.statusBadge("garbage"), {
		label: "未知",
		tone: "neutral",
	});
});

test("groupWorks buckets the seven statuses into the three fixed groups", () => {
	const records = [
		{ id: "a", status: "running" },
		{ id: "b", status: "needs_human" },
		{ id: "c", status: "failed" },
		{ id: "d", status: "blocked" },
		{ id: "e", status: "completed" },
		{ id: "f", status: "cancelled" },
		{ id: "g", status: "queued" },
	];
	const groups = WorkView.groupWorks(records);
	assert.deepEqual(
		groups.map((group) => group.title),
		["需要你", "进行中", "最近完成"],
	);
	assert.deepEqual(
		groups.map((group) => group.items.map((item) => item.id)),
		[["b", "d"], ["a", "g"], ["c", "e", "f"]],
	);
	// 组内顺序保持后端排序（updated_at DESC），不做二次排序
	const ordered = WorkView.groupWorks([
		{ id: "new", status: "running", updated_at: "2026-08-27T10:00:00Z" },
		{ id: "old", status: "running", updated_at: "2026-08-26T10:00:00Z" },
	]);
	assert.deepEqual(
		ordered[1].items.map((item) => item.id),
		["new", "old"],
	);
});

test("relativeTime degrades from 刚刚 to absolute date", () => {
	const now = Date.parse("2026-08-27T12:00:00Z");
	assert.equal(WorkView.relativeTime("2026-08-27T11:59:40Z", now), "刚刚");
	assert.equal(
		WorkView.relativeTime("2026-08-27T11:35:00Z", now),
		"25 分钟前",
	);
	assert.equal(WorkView.relativeTime("2026-08-27T06:00:00Z", now), "6 小时前");
	assert.equal(WorkView.relativeTime("2026-08-24T12:00:00Z", now), "3 天前");
	assert.equal(WorkView.relativeTime("2026-06-01T12:00:00Z", now), "2026-06-01");
	assert.equal(WorkView.relativeTime("", now), "");
	assert.equal(WorkView.relativeTime("not-a-date", now), "");
});

test("detailModel composes sections from record, events and receipt probe", () => {
	const record = {
		id: "work-1",
		title: "调研 durable work",
		intent: "帮我调研并总结",
		status: "needs_human",
		summary: "完成了调研",
		nextAction: "等待人工验收",
		artifactUri: "agentwork://abc123/work-abc",
		receiptRef: "local-run-1",
	};
	const events = [
		{
			action: "work.status_changed",
			created_at: "2026-08-27T10:00:00Z",
			payload: { from: "running", to: "needs_human", reason: "artifact" },
		},
		{
			action: "work.created",
			created_at: "2026-08-27T09:00:00Z",
			payload: { status: "queued" },
		},
	];
	const detail = WorkView.detailModel(record, events, { exists: true });
	assert.equal(detail.status.label, "需要你");
	assert.equal(detail.intent, "帮我调研并总结");
	assert.deepEqual(detail.statusLine, {
		text: "由 进行中 转为 需要你",
		reason: "artifact",
	});
	assert.equal(detail.nextAction, "等待人工验收");
	assert.deepEqual(detail.artifact, {
		uri: "agentwork://abc123/work-abc",
		id: "work-abc",
	});
	assert.deepEqual(detail.receipt, { ref: "local-run-1", exists: true });
	assert.equal(detail.canAccept, true);
	assert.equal(detail.canCancel, false);
	assert.equal(detail.events.length, 2);
	assert.equal(detail.events[0].reason, "artifact");
});

test("detailModel drives actions: accept only needs_human, cancel only queued/running", () => {
	const base = { receiptRef: null, artifactUri: null, nextAction: "" };
	for (const [status, canAccept, canCancel] of [
		["queued", false, true],
		["running", false, true],
		["needs_human", true, false],
		["blocked", false, false],
		["completed", false, false],
		["failed", false, false],
		["cancelled", false, false],
	]) {
		const detail = WorkView.detailModel({ ...base, status }, [], null);
		assert.equal(
			detail.canAccept,
			canAccept,
			`${status} accept 期望 ${canAccept}`,
		);
		assert.equal(
			detail.canCancel,
			canCancel,
			`${status} cancel 期望 ${canCancel}`,
		);
	}
});

test("detailModel stays renderable without events or artifact", () => {
	const detail = WorkView.detailModel(
		{ id: "w", title: "", intent: "", status: "queued" },
		[],
		null,
	);
	assert.equal(detail.statusLine, null);
	assert.equal(detail.artifact, null);
	assert.equal(detail.receipt.exists, null);
	assert.equal(detail.title, "未命名工作");
});

test("rowModel exposes artifact for in-row shortcut", () => {
	const row = WorkView.rowModel({
		id: "w1",
		title: "t",
		status: "needs_human",
		artifactUri: "agentwork://abc123/work-1",
		updatedAt: "2026-08-28T00:00:00Z",
	});
	assert.deepEqual(row.artifact, { uri: "agentwork://abc123/work-1", id: "work-1" });
	const none = WorkView.rowModel({
		id: "w2",
		title: "t",
		status: "queued",
		artifactUri: null,
		updatedAt: "2026-08-28T00:00:00Z",
	});
	assert.equal(none.artifact, null);
});

test("artifactWorkId parses canonical agentwork URIs only", () => {
	assert.equal(WorkView.artifactWorkId("agentwork://key/work-1"), "work-1");
	assert.equal(WorkView.artifactWorkId("agent://key/work-1"), null);
	assert.equal(WorkView.artifactWorkId("workspace://a.md"), null);
	assert.equal(WorkView.artifactWorkId(""), null);
});

test("rowMatches filters by title/intent/summary", () => {
	const record = {
		title: "调研 durable work",
		intent: "帮我调研并总结 durable work",
		summary: "完成了调研",
	};
	assert.equal(WorkView.rowMatches(record, "durable"), true);
	assert.equal(WorkView.rowMatches(record, "DURABLE"), true);
	assert.equal(WorkView.rowMatches(record, "不存在"), false);
	assert.equal(WorkView.rowMatches(record, ""), true);
});

test("activeCount counts only queued/running for polling decisions", () => {
	assert.equal(
		WorkView.activeCount([
			{ status: "running" },
			{ status: "queued" },
			{ status: "needs_human" },
		]),
		2,
	);
	assert.equal(WorkView.activeCount([{ status: "failed" }]), 0);
});
