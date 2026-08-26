// ui/feeds.js — RSS 源列表的纯展示逻辑（不依赖 DOM，可单测）。
// 状态与渲染仍在 app.js；这里只放可以从字符串/数字推导的展示规则。
(function attachFeeds(window) {
	function formatFetchTime(value) {
		if (!value) return "";
		const millis = value < 100000000000 ? value * 1000 : value;
		return new Date(millis).toLocaleString("zh-CN", {
			month: "numeric",
			day: "numeric",
			hour: "2-digit",
			minute: "2-digit",
		});
	}

	// 源状态：错误优先，其次成功时间，从未抓取则提示。
	function feedSourceStatusText(source) {
		if (source.last_error) return `失败：${source.last_error}`;
		if (source.last_fetch_at)
			return `成功 · ${formatFetchTime(source.last_fetch_at)}`;
		return "尚未抓取";
	}

	// RSS 区块计数：`N 源 · M 篇`（M 来自 Library source 视图）。
	function rssSourceCountText(feedSources, rssLibrarySource) {
		if (!feedSources?.length) return "还没有源";
		const documents = rssLibrarySource?.documents || 0;
		return `${feedSources.length} 源 · ${documents.toLocaleString()} 篇`;
	}

	window.StillwriteFeeds = { feedSourceStatusText, rssSourceCountText };
})(typeof window !== "undefined" ? window : globalThis);
