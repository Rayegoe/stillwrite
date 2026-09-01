(function exposeFileMenu(root, factory) {
	if (typeof module === "object" && module.exports) module.exports = factory();
	else root.StillwriteFileMenu = factory();
})(typeof globalThis === "object" ? globalThis : window, () => {
	// File menu 的纯投影逻辑（02_FILE_MENU_UX_SPEC §3 / §5）：
	// DOM 只投影这里的返回值，菜单本身不缓存 root / document / recent 状态。

	/**
	 * 当前上下文下 File menu 各项的启用状态（menu state matrix）。
	 * @param {object} ctx
	 * @param {boolean} ctx.hasWorkspace      rootPath 是否存在
	 * @param {string|null} ctx.currentFile   Workspace Markdown 路径
	 * @param {object|null} ctx.libraryDoc    Library 文档
	 * @param {object|null} ctx.agentDoc      Agent Work 文档
	 * @returns {{newFile: boolean, open: boolean, recent: boolean, save: boolean, saveAs: boolean, workspace: boolean}}
	 */
	function menuState(ctx = {}) {
		const hasWorkspace = Boolean(ctx.hasWorkspace);
		const hasWorkspaceDoc = Boolean(ctx.currentFile) && !ctx.libraryDoc;
		const hasAgentDoc = Boolean(ctx.agentDoc);
		return {
			// 新建 Markdown：有 Workspace 直接弹对话框；无 Workspace 先选文件夹再创建
			newFile: true,
			open: true,
			recent: true,
			// 保存：Workspace Markdown / Agent Work Markdown
			save: hasWorkspaceDoc || hasAgentDoc,
			// 另存为：v0.1 仅 Workspace Markdown
			saveAs: hasWorkspaceDoc,
			// 工作区子菜单：只有 rootPath 存在时可用
			workspace: hasWorkspace,
		};
	}

	/**
	 * 后端 recent_locations 行 → 菜单项模型。不可用路径不禁用展示、
	 * 只禁用点击，并在标签后标注（不可用）；不自动删除记录。
	 * @param {{id: number, kind: string, path: string, label: string, available: boolean, openedAt: string}} location
	 */
	function recentItemModel(location) {
		const kind = location.kind === "workspace" ? "workspace" : "document";
		const available = Boolean(location.available);
		return {
			id: location.id,
			kind,
			path: location.path,
			label: location.label || location.path,
			available,
			disabled: !available,
			displayLabel: available
				? location.label || location.path
				: `${location.label || location.path}（不可用）`,
			icon: kind === "workspace" ? "▣" : "▤",
			title: location.path,
		};
	}

	/**
	 * 下一个/上一个 enabled 菜单项下标。找不到时返回 -1。
	 * @param {Array<{disabled?: boolean, hidden?: boolean}>} items
	 * @param {number} currentIndex 当前焦点下标（-1 表示无）
	 * @param {1|-1} direction
	 */
	function nextEnabledMenuItem(items, currentIndex, direction) {
		if (!Array.isArray(items) || items.length === 0) return -1;
		const step = direction >= 0 ? 1 : -1;
		let index = currentIndex;
		for (let visited = 0; visited < items.length; visited++) {
			index = (index + step + items.length) % items.length;
			const item = items[index];
			if (item && !item.disabled && !item.hidden) return index;
		}
		return -1;
	}

	/**
	 * 第一个/最后一个 enabled 菜单项下标（Home / End）。
	 */
	function edgeEnabledMenuItem(items, edge) {
		if (!Array.isArray(items) || items.length === 0) return -1;
		const order = edge === "end" ? [...items].reverse() : items;
		const found = order.findIndex(
			(item) => item && !item.disabled && !item.hidden,
		);
		if (found === -1) return -1;
		return edge === "end" ? items.length - 1 - found : found;
	}

	return {
		menuState,
		recentItemModel,
		nextEnabledMenuItem,
		edgeEnabledMenuItem,
	};
});
