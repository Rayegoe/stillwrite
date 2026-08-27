(function exposeWorkView(root, factory) {
	if (typeof module === "object" && module.exports) module.exports = factory();
	else root.StillwriteWorkView = factory();
})(typeof globalThis === "object" ? globalThis : window, () => {
	// Work Home 的三组固定映射（02 M2/M3）：组内保持后端排序（updated_at DESC）。
	const WORK_GROUPS = [
		{ key: "needsYou", title: "需要你", statuses: ["needs_human", "blocked"] },
		{ key: "inProgress", title: "进行中", statuses: ["queued", "running"] },
		{
			key: "recentlyDone",
			title: "最近完成",
			statuses: ["completed", "failed", "cancelled"],
		},
	];

	// badge 必须带文字（可访问性：不只用颜色）。
	const STATUS_BADGES = {
		queued: { label: "排队", tone: "neutral" },
		running: { label: "进行中", tone: "blue" },
		needs_human: { label: "需要你", tone: "orange" },
		blocked: { label: "受阻", tone: "amber" },
		completed: { label: "完成", tone: "green" },
		failed: { label: "失败", tone: "red" },
		cancelled: { label: "已取消", tone: "gray" },
	};

	function statusBadge(status) {
		return STATUS_BADGES[status] || { label: "未知", tone: "neutral" };
	}

	function groupWorks(records) {
		return WORK_GROUPS.map((group) => ({
			key: group.key,
			title: group.title,
			items: records.filter((record) =>
				group.statuses.includes(record.status),
			),
		}));
	}

	function activeCount(records) {
		return records.filter((record) =>
			["queued", "running"].includes(record.status),
		).length;
	}

	function relativeTime(iso, now = Date.now()) {
		const time = Date.parse(String(iso || ""));
		if (!Number.isFinite(time)) return "";
		const seconds = Math.max(0, Math.floor((now - time) / 1000));
		if (seconds < 60) return "刚刚";
		const minutes = Math.floor(seconds / 60);
		if (minutes < 60) return `${minutes} 分钟前`;
		const hours = Math.floor(minutes / 60);
		if (hours < 24) return `${hours} 小时前`;
		const days = Math.floor(hours / 24);
		if (days < 7) return `${days} 天前`;
		const date = new Date(time);
		const month = String(date.getMonth() + 1).padStart(2, "0");
		const day = String(date.getDate()).padStart(2, "0");
		return `${date.getFullYear()}-${month}-${day}`;
	}

	function rowModel(record, now = Date.now()) {
		const badge = statusBadge(record.status);
		return {
			id: record.id,
			title: record.title || "未命名工作",
			status: record.status,
			badge,
			summary: record.summary || "",
			nextAction: record.nextAction || "",
			updatedAt: record.updatedAt,
			relativeTime: relativeTime(record.updatedAt, now),
			artifactUri: record.artifactUri || null,
			artifact: record.artifactUri
				? { uri: record.artifactUri, id: artifactWorkId(record.artifactUri) }
				: null,
		};
	}

	// `agentwork://<workspace-key>/<agent-work-id>` → agent-work-id。
	function artifactWorkId(uri) {
		const match = /^agentwork:\/\/[^/]+\/(.+)$/.exec(String(uri || ""));
		return match ? match[1] : null;
	}

	// events 为倒序（后端 id DESC）；取最近一次状态转换。
	function lastTransition(events) {
		const event = (events || []).find(
			(item) => item.action === "work.status_changed",
		);
		if (!event) return null;
		const payload = event.payload || {};
		if (!payload.from || !payload.to) return null;
		return {
			from: statusBadge(String(payload.from)),
			to: statusBadge(String(payload.to)),
			reason: typeof payload.reason === "string" ? payload.reason : "",
			at: event.created_at,
		};
	}

	function detailModel(record, events = [], receipt = null) {
		const transition = lastTransition(events);
		const status = statusBadge(record.status);
		const eventModels = (events || []).slice(0, 10).map((event) => ({
			action: event.action,
			time: relativeTime(event.created_at),
			reason:
				event.payload && typeof event.payload.reason === "string"
					? event.payload.reason
					: "",
		}));
		return {
			id: record.id,
			title: record.title || "未命名工作",
			status,
			intent: record.intent || record.title || "",
			summary: record.summary || "",
			statusLine: transition
				? {
						text: `由 ${transition.from.label} 转为 ${transition.to.label}`,
						reason: transition.reason,
					}
				: null,
			nextAction: record.nextAction || "",
			artifact: record.artifactUri
				? { uri: record.artifactUri, id: artifactWorkId(record.artifactUri) }
				: null,
			receipt: {
				ref: record.receiptRef || "",
				exists:
					receipt && typeof receipt.exists === "boolean"
						? receipt.exists
						: null,
			},
			events: eventModels,
			canAccept: record.status === "needs_human",
			canCancel: ["queued", "running"].includes(record.status),
		};
	}

	function rowMatches(record, query) {
		const needle = String(query || "").trim().toLocaleLowerCase();
		if (!needle) return true;
		return [record.title, record.intent, record.summary]
			.filter(Boolean)
			.join("\n")
			.toLocaleLowerCase()
			.includes(needle);
	}

	return {
		WORK_GROUPS,
		STATUS_BADGES,
		artifactWorkId,
		activeCount,
		detailModel,
		groupWorks,
		lastTransition,
		relativeTime,
		rowMatches,
		rowModel,
		statusBadge,
	};
});
