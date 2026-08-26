const test = require("node:test");
const assert = require("node:assert/strict");
const AgentEvents = require("./agent-events.js");

test("reduces text deltas into a running preview", () => {
	let run = { id: "r1", status: "running", streamText: "" };
	run = AgentEvents.applyAgentEvent(run, { type: "message_update", delta: "hello " });
	run = AgentEvents.applyAgentEvent(run, { type: "message_update", delta: "world" });
	assert.equal(run.streamText, "hello world");
	assert.equal(run.preview, "hello world");
	assert.equal(run.status, "生成中");
});

test("settlement uses the authoritative final text and becomes terminal", () => {
	const run = AgentEvents.applyAgentEvent(
		{ id: "r1", streamText: "partial", status: "生成中" },
		{ type: "agent_settled", text: "authoritative", piSessionRef: "session.jsonl" },
	);
	assert.equal(run.streamText, "authoritative");
	assert.equal(run.finalText, "authoritative");
	assert.equal(run.piSessionRef, "session.jsonl");
	assert.equal(run.status, "已完成");
	assert.equal(run.terminal, true);
});

test("stop and error are terminal but do not fabricate final text", () => {
	const stopped = AgentEvents.applyAgentEvent(
		{ id: "r1", streamText: "partial" },
		{ type: "agent_stopped" },
	);
	const failed = AgentEvents.applyAgentEvent(
		{ id: "r2", streamText: "partial" },
		{ type: "error", message: "Pi exited" },
	);
	assert.equal(stopped.status, "已停止");
	assert.equal(stopped.finalText, undefined);
	assert.equal(failed.status, "失败");
	assert.equal(failed.error, "Pi exited");
	assert.equal(failed.terminal, true);
});
