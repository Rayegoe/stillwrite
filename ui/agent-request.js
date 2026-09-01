// Agent 请求契约（M4 Interaction Contract Reset）的纯函数模型。
// instruction = 人的原始要求；context = 当前文档/选区/引用的结构化或
// 已编译输入；mode = assist | work 决定是否落地 durable Work。
// 三者在此显式分开，任何阶段都不得再拼成一个 prompt 领域字段。
// DOM 接线在 app.js；这里只做可测试的构造与校验。

(function exposeAgentRequest(root, factory) {
	if (typeof module === "object" && module.exports) module.exports = factory();
	else root.StillwriteAgentRequest = factory();
})(typeof globalThis === "object" ? globalThis : window, () => {
	const MODES = ["assist", "work"];

	// 展示标题：取 instruction 首行，剥掉 Markdown 标记与结尾标点。
	// 仅用于列表/会话名展示，永不回写为领域 intent。
	function displayTitle(instruction) {
		const firstLine =
			String(instruction || "")
				.split(/\r?\n/)
				.map((line) => line.trim())
				.find(Boolean) || "Agent 工作";
		const clean = firstLine.replace(/^#+\s*/, "").replace(/[。！？.!?]+$/, "");
		return clean.length > 56 ? `${clean.slice(0, 56)}…` : clean;
	}

	// 组装 agent_start 的请求体。instruction 必填；mode 缺省 assist
	// （宁可少建 Work，不让普通提问污染 Work Inbox）。
	function buildStartInput({
		mode = "assist",
		runId,
		instruction,
		originUri = null,
		originQuote = null,
		citationContext = "",
	} = {}) {
		const normalizedMode = MODES.includes(mode) ? mode : "assist";
		const text = String(instruction || "").trim();
		if (!text) throw new Error("Agent 指令不能为空");
		if (!runId) throw new Error("Agent run id 不能为空");
		return {
			mode: normalizedMode,
			runId,
			instruction: text,
			title: displayTitle(text),
			context: {
				originUri: originUri || null,
				originQuote: originQuote || null,
				citationContext: String(citationContext || ""),
			},
		};
	}

	return { MODES, displayTitle, buildStartInput };
});
