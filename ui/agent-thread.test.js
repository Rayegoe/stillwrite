"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const AgentThread = require("./agent-thread.js");

const MESSAGES = [
	{
		id: "u1",
		role: "user",
		content: "Work-first 有什么问题？",
		runRef: "run-1",
		originUri: "workspace://docs/a.md",
		quoteSnapshot: "Work 应成为默认首页。",
	},
	{
		id: "a1",
		role: "assistant",
		content: "它把协调层提升成了主交互面。",
		runRef: "run-1",
	},
	{
		id: "u2",
		role: "user",
		content: "那 Work 应该放在哪里？",
		runRef: "run-2",
		originUri: null,
		quoteSnapshot: null,
	},
];

test("buildTurns 把 user/assistant 消息按 runRef 配对成 turn", () => {
	const turns = AgentThread.buildTurns(MESSAGES, new Map());
	assert.equal(turns.length, 2);
	assert.equal(turns[0].instruction, "Work-first 有什么问题？");
	assert.equal(turns[0].quote, "Work 应成为默认首页。");
	assert.equal(turns[0].answer.state, "done");
	assert.equal(turns[0].answerMessageId, "a1");
	// 尾部无回答且无 live run → missing（运行失败/取消/中断后的样子）
	assert.equal(turns[1].answer.state, "missing");
});

test("buildTurns 用 live run 填充流式槽，不重放已完成内容", () => {
	const liveRuns = new Map([
		["run-2", { streamText: "应该作为后台委派层", terminal: false }],
	]);
	const turns = AgentThread.buildTurns(MESSAGES, liveRuns);
	assert.equal(turns[1].answer.state, "streaming");
	assert.equal(turns[1].answer.content, "应该作为后台委派层");
});

test("buildTurns 把 terminal 的 live run 投影为失败", () => {
	const liveRuns = new Map([
		["run-2", { streamText: "", terminal: true, status: "失败", error: "Pi 拒绝" }],
	]);
	const turns = AgentThread.buildTurns(MESSAGES, liveRuns);
	assert.equal(turns[1].answer.state, "failed");
	assert.equal(turns[1].answer.error, "Pi 拒绝");
});

test("partitionSessions 三分列表且当前会话优先", () => {
	const sessions = [
		{ id: "s1", lastOriginUri: "workspace://docs/a.md" },
		{ id: "s2", lastOriginUri: "workspace://docs/b.md" },
		{ id: "s3", lastOriginUri: "workspace://docs/a.md" },
	];
	const { current, related, recent } = AgentThread.partitionSessions(sessions, {
		activeSessionId: "s1",
		originUri: "workspace://docs/a.md",
	});
	assert.equal(current.id, "s1");
	assert.deepEqual(related.map((s) => s.id), ["s3"]);
	assert.deepEqual(recent.map((s) => s.id), ["s2"]);
	// 无当前文档 → 全部进 recent，历史不丢
	const noOrigin = AgentThread.partitionSessions(sessions, {
		activeSessionId: "s1",
	});
	assert.equal(noOrigin.related.length, 0);
	assert.equal(noOrigin.recent.length, 2);
});

test("sessionRowMeta 输出条数摘要", () => {
	assert.equal(AgentThread.sessionRowMeta({ messageCount: 6 }), "6 条");
	assert.equal(AgentThread.sessionRowMeta(null), "");
});
