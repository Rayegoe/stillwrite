(function exposeAgentEvents(root, factory) {
	if (typeof module === "object" && module.exports) module.exports = factory();
	else root.StillwriteAgentEvents = factory();
})(typeof globalThis === "object" ? globalThis : window, () => {
	const TERMINAL_EVENTS = new Set(["agent_settled", "agent_stopped", "error"]);

	function compactPreview(value, limit = 360) {
		const text = String(value || "").replace(/\s+/g, " ").trim();
		return text.length > limit ? `${text.slice(0, limit)}…` : text;
	}

	function touched(run) {
		return { ...run, updatedAt: Math.floor(Date.now() / 1000) };
	}

	function applyAgentEvent(run, event) {
		const next = { ...run };
		const type = event?.type;
		if (!type) return next;
		if (type === "agent_start") {
			next.status = "运行中";
			next.terminal = false;
			return touched(next);
		}
		if (type === "message_update") {
			const delta = String(event.delta || "");
			next.streamText = `${next.streamText || ""}${delta}`;
			next.preview = compactPreview(next.streamText);
			next.status = "生成中";
			next.terminal = false;
			return touched(next);
		}
		if (type === "tool_execution_start") {
			next.toolStatus = ["workspace_list", "workspace_read", "workspace_search"].includes(event.toolName)
				? "读取 Workspace"
				: "查看资料";
			next.status = next.toolStatus;
			next.terminal = false;
			return touched(next);
		}
		if (type === "tool_execution_end") {
			next.toolStatus = "";
			if (!next.terminal) next.status = "生成中";
			return touched(next);
		}
		if (type === "compaction_start") {
			next.status = "整理上下文";
			next.compacting = true;
			return touched(next);
		}
		if (type === "compaction_end") {
			next.compacting = false;
			if (!next.terminal) next.status = "生成中";
			return touched(next);
		}
		if (type === "agent_settled") {
			const text = String(event.text || "");
			next.finalText = text;
			next.streamText = text;
			next.preview = compactPreview(text);
			next.piSessionRef = event.piSessionRef || next.piSessionRef || null;
			next.status = "已完成";
			next.terminal = true;
			next.toolStatus = "";
			return touched(next);
		}
		if (type === "agent_stopped") {
			next.status = "已停止";
			next.terminal = true;
			next.toolStatus = "";
			return touched(next);
		}
		if (type === "error") {
			next.status = "失败";
			next.error = String(event.message || "Agent 运行失败");
			next.terminal = true;
			next.toolStatus = "";
			return touched(next);
		}
		return next;
	}

	function finalizeAgentRun(run, finalText) {
		return applyAgentEvent(run, {
			type: "agent_settled",
			text: finalText,
			piSessionRef: run?.piSessionRef || null,
		});
	}

	return { applyAgentEvent, compactPreview, finalizeAgentRun, TERMINAL_EVENTS };
});
