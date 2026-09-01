// Agent Thread 纯投影模块（M5 Durable Agent Thread）。
// durable agent_messages（已按时间有序）+ 进行中的 run（流式/失败状态）
// → 右栏 thread 的 turn 模型 + 会话列表分组。
// 纯函数；DOM 接线在 app.js。

(function exposeAgentThread(root, factory) {
	if (typeof module === "object" && module.exports) module.exports = factory();
	else root.StillwriteAgentThread = factory();
})(typeof globalThis === "object" ? globalThis : window, () => {
	// 把 durable messages + live runs 合成为 turn 序列。
	// user message 开启一个 turn；同 runRef 的 assistant message 归入该 turn。
	// 没有 assistant 的 turn：有 live run → streaming/failed；否则 → missing。
	//
	// messages: [{ id, role, content, runRef, originUri, quoteSnapshot }]
	// liveRuns: Map(runRef → { streamText, terminal, status, error })
	function buildTurns(messages, liveRuns = new Map()) {
		const turns = [];
		for (const message of messages || []) {
			if (message.role === "user") {
				turns.push({
					key: message.id,
					instruction: message.content,
					originUri: message.originUri || null,
					quote: message.quoteSnapshot || null,
					runRef: message.runRef || null,
					createdAt: message.createdAt || null,
					answerMessageId: null,
					answer: null,
				});
				continue;
			}
			const last = turns[turns.length - 1];
			if (
				last &&
				!last.answer &&
				message.runRef &&
				last.runRef === message.runRef
			) {
				last.answer = { state: "done", content: message.content };
				last.answerMessageId = message.id;
			}
			// 无属主的 assistant message（理论不应出现）在此忽略，不冒充 turn。
		}
		for (const turn of turns) {
			if (turn.answer) continue;
			const run = turn.runRef ? liveRuns.get(turn.runRef) : null;
			if (run && !run.terminal) {
				turn.answer = { state: "streaming", content: run.streamText || "" };
			} else if (run && run.terminal) {
				turn.answer = {
					state: "failed",
					content: "",
					status: run.status || "失败",
					error: run.error || null,
				};
			} else {
				turn.answer = { state: "missing", content: "" };
			}
		}
		return turns;
	}

	// 会话列表三分：当前展开 / 与当前文档相关 / 其余最近。
	// 切换文档只改变排序，永不删除会话（历史必须一直在）。
	function partitionSessions(
		sessions,
		{ originUri = null, activeSessionId = null, limit = 8 } = {},
	) {
		const all = sessions || [];
		const current = all.find((item) => item.id === activeSessionId) || null;
		const rest = all.filter((item) => item.id !== activeSessionId);
		const related = originUri
			? rest.filter((item) => item.lastOriginUri === originUri)
			: [];
		const relatedIds = new Set(related.map((item) => item.id));
		const recent = rest.filter((item) => !relatedIds.has(item.id));
		return {
			current,
			related: related.slice(0, limit),
			recent: recent.slice(0, limit),
		};
	}

	// 会话行摘要：`6 次问答 · 刚刚` 中的前半由 count 提供。
	function sessionRowMeta(summary) {
		if (!summary) return "";
		const count = Number(summary.messageCount || 0);
		return `${count} 条`;
	}

	return { buildTurns, partitionSessions, sessionRowMeta };
});
