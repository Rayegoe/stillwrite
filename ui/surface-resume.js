// 启动恢复（M4）：Resume Last Human Context 的纯决策模块。
// lastHumanSurface 是 projection preference（AGENTS.md 2.1 允许存
// localStorage）；domain truth 仍在 state.db / 文件，恢复前必须先探测可用性。
//
// 恢复优先级（02_M3_HANDOFF）：
//   1. 上次有效的 Human Surface（workspace 文档 / Agent Artifact / Library / Work）；
//   2. 否则恢复最近可用的 Workspace 文档；
//   3. 再无可恢复对象 → Work Home / empty state。

(function exposeSurfaceResume(root, factory) {
	if (typeof module === "object" && module.exports) module.exports = factory();
	else root.StillwriteSurfaceResume = factory();
})(typeof globalThis === "object" ? globalThis : window, () => {
	const STORAGE_KEY = "stillwrite.lastSurface";
	const TYPES = ["workspace", "artifact", "library", "work"];

	// 读取上次 Human Surface；键缺失、损坏或类型未知 → null（按无记录处理，
	// 不抛错——启动路径不允许因为一条坏偏好而失败）。
	function read(storage) {
		const raw = storage.getItem(STORAGE_KEY);
		if (!raw) return null;
		try {
			const surface = JSON.parse(raw);
			if (!surface || !TYPES.includes(surface.type)) return null;
			return surface;
		} catch {
			return null;
		}
	}

	// 记录当前 Human Surface。type=work 时 id 为 null 表示 Work Home 本身。
	function write(storage, surface) {
		if (!surface || !TYPES.includes(surface.type)) return;
		try {
			storage.setItem(STORAGE_KEY, JSON.stringify(surface));
		} catch {
			// 存储不可写时静默跳过：记录 projection 失败不影响当前使用。
		}
	}

	// 纯决策：按优先级产出恢复计划。
	//   surface               上次 Human Surface（可为 null）
	//   surfaceAvailable      上次对象经探测仍然可用（无记录时忽略）
	//   fallbackDocument      最近可用 Workspace 文档路径（可为 null）
	// 返回：
	//   { kind: "surface", surface }      原样恢复上次 Surface
	//   { kind: "fallback-document", path } 恢复最近可用文档
	//   { kind: "work-home" }             进入 Work Home / empty state
	function planResume({ surface = null, surfaceAvailable = false, fallbackDocument = null }) {
		if (surface && surfaceAvailable) return { kind: "surface", surface };
		if (fallbackDocument) return { kind: "fallback-document", path: fallbackDocument };
		return { kind: "work-home" };
	}

	return { STORAGE_KEY, TYPES, read, write, planResume };
});
