"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const AgentRequest = require("./agent-request.js");

test("displayTitle 取 instruction 首行并剥掉标记与结尾标点", () => {
	assert.equal(AgentRequest.displayTitle("帮我查证这个判断。\n第二行"), "帮我查证这个判断");
	assert.equal(AgentRequest.displayTitle("# 标题先行"), "标题先行");
	assert.equal(
		AgentRequest.displayTitle(`${"长".repeat(60)}超出部分被截断`),
		`${"长".repeat(56)}…`,
	);
	assert.equal(AgentRequest.displayTitle(""), "Agent 工作");
	assert.equal(AgentRequest.displayTitle(null), "Agent 工作");
});

test("buildStartInput 缺省 assist 且分离 instruction 与 context", () => {
	const input = AgentRequest.buildStartInput({
		runId: "local-abc",
		instruction: "  查证这个判断，并找两个反例  ",
		originUri: "workspace:///a.md",
		originQuote: "原文",
		citationContext: "### 资料",
	});
	assert.equal(input.mode, "assist");
	assert.equal(input.instruction, "查证这个判断，并找两个反例");
	assert.equal(input.title, "查证这个判断，并找两个反例");
	assert.deepEqual(input.context, {
		originUri: "workspace:///a.md",
		originQuote: "原文",
		citationContext: "### 资料",
	});
	// 契约里不存在 prompt 领域字段
	assert.ok(!("prompt" in input));
});

test("buildStartInput 显式 work 保持 work", () => {
	const input = AgentRequest.buildStartInput({
		mode: "work",
		runId: "local-abc",
		instruction: "整理这份资料并写对比草稿",
	});
	assert.equal(input.mode, "work");
	assert.equal(input.title, "整理这份资料并写对比草稿");
});

test("buildStartInput 未知 mode 回退 assist，空 instruction 拒绝", () => {
	const fallback = AgentRequest.buildStartInput({
		mode: "delegate",
		runId: "local-abc",
		instruction: "问一句",
	});
	assert.equal(fallback.mode, "assist");
	assert.throws(
		() => AgentRequest.buildStartInput({ runId: "local-abc", instruction: "   " }),
		/指令不能为空/,
	);
	assert.throws(
		() => AgentRequest.buildStartInput({ instruction: "问" }),
		/run id/,
	);
});
