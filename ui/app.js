const invoke = window.__TAURI__.core.invoke;
const listen = window.__TAURI__.event?.listen;
const openUrl = window.__TAURI__.opener?.openUrl;

const shell = document.querySelector("#shell");
const formatToolbar = document.querySelector("#formatToolbar");
const headingMenu = document.querySelector("#headingMenu");
const headingButton = document.querySelector('[data-format-command="heading"]');
const formatButtons = new Map(
	[...formatToolbar.querySelectorAll(".format-btn")].map((btn) => [
		btn.dataset.formatCommand,
		btn,
	]),
);
const bodyClassList = document.body.classList;
const sidebar = document.querySelector("#sidebar");
const sidebarHandle = document.querySelector("#sidebarHandle");
const paneHandle = document.querySelector("#paneHandle");
const panes = document.querySelector("#panes");
const editorPane = document.querySelector("#editorPane");
const readerPane = document.querySelector("#readerPane");
const editor = document.querySelector("#editor");
const editorPaneLabel = document.querySelector("#editorPaneLabel");
const treeEl = document.querySelector("#tree");
const previewEl = document.querySelector("#preview");
const saveStateEl = document.querySelector("#saveState");
const documentTitleEl = document.querySelector("#documentTitle");
const workspaceNameEl = document.querySelector("#workspaceName");
const newFileDialog = document.querySelector("#newFileDialog");
const newFileName = document.querySelector("#newFileName");
const newFileForm = document.querySelector("#newFileForm");
const syncButton = document.querySelector("#syncButton");
const syncStateEl = document.querySelector("#syncState");
const searchInput = document.querySelector("#searchInput");
const workspaceTab = document.querySelector("#workspaceTab");
const libraryTab = document.querySelector("#libraryTab");
const workTabRef = document.querySelector("#workTab");
const legacyAgentWorksToggle = document.querySelector(
	"#legacyAgentWorksToggle",
);
const workPane = document.querySelector("#workPane");
const workHome = document.querySelector("#workHome");
const workDetail = document.querySelector("#workDetail");
const workContextViewButton = document.querySelector("#workContextViewButton");
const workEvidenceViewButton = document.querySelector(
	"#workEvidenceViewButton",
);
const workContextStream = document.querySelector("#workContextStream");
const workEvidenceStream = document.querySelector("#workEvidenceStream");
const workEvidenceCount = document.querySelector("#workEvidenceCount");
const addLibrarySourceButton = document.querySelector("#addLibrarySource");
const newAgentWorkButton = document.querySelector("#newAgentWorkButton");
const libraryMeta = document.querySelector("#libraryMeta");
const libraryStats = document.querySelector("#libraryStats");
const feedStatusLine = document.querySelector("#feedStatusLine");
const sourceMenu = document.querySelector("#sourceMenu");
const addLibrarySourceMenuItem = document.querySelector(
	"#addLibrarySourceMenuItem",
);
const addFeedMenuItem = document.querySelector("#addFeedMenuItem");
const importOpmlMenuItem = document.querySelector("#importOpmlMenuItem");
const addFeedDialog = document.querySelector("#addFeedDialog");
const addFeedForm = document.querySelector("#addFeedForm");
const addFeedUrl = document.querySelector("#addFeedUrl");
const cancelAddFeed = document.querySelector("#cancelAddFeed");
const workMetaEl = document.querySelector("#workMeta");
const workStatsEl = document.querySelector("#workStats");
const worksetBar = document.querySelector("#worksetBar");
const worksetCount = document.querySelector("#worksetCount");
const clearWorksetButton = document.querySelector("#clearWorkset");
const fileMenuRoot = document.querySelector("#fileMenuRoot");
const fileMenuButton = document.querySelector("#fileMenuButton");
const fileMenu = document.querySelector("#fileMenu");
const newFileItem = document.querySelector("#newFile");
const openFolderItem = document.querySelector("#openFolder");
const openDocumentItem = document.querySelector("#openDocument");
const saveDocumentItem = document.querySelector("#saveDocument");
const saveDocumentAsItem = document.querySelector("#saveDocumentAs");
const recentMenuTrigger = document.querySelector("#recentMenuTrigger");
const recentMenu = document.querySelector("#recentMenu");
const workspaceMenuTrigger = document.querySelector("#workspaceMenuTrigger");
const workspaceMenu = document.querySelector("#workspaceMenu");
const workspaceRevealItem = document.querySelector("#workspaceReveal");
const workspaceRefreshItem = document.querySelector("#workspaceRefresh");
const workspaceAggregateItem = document.querySelector("#workspaceAggregate");
const workspaceCloseItem = document.querySelector("#workspaceClose");
const treeContextMenu = document.querySelector("#treeContextMenu");
const newFolderDialog = document.querySelector("#newFolderDialog");
const newFolderForm = document.querySelector("#newFolderForm");
const newFolderName = document.querySelector("#newFolderName");
const cancelNewFolderButton = document.querySelector("#cancelNewFolder");
const annotationButton = document.querySelector("#annotationButton");
const annotatePanel = document.querySelector("#annotatePanel");
const annotateHandle = document.querySelector("#annotateHandle");
const closeAnnotate = document.querySelector("#closeAnnotate");
const aggregateButton = document.querySelector("#aggregateButton");
const annotateDocName = document.querySelector("#annotateDocName");
const annotateDocPath = document.querySelector("#annotateDocPath");
const annotateCount = document.querySelector("#annotateCount");
const relatedCount = document.querySelector("#relatedCount");
const webSearchCount = document.querySelector("#webSearchCount");
const annotationViewButton = document.querySelector("#annotationViewButton");
const relatedViewButton = document.querySelector("#relatedViewButton");
const webSearchViewButton = document.querySelector("#webSearchViewButton");
const annotateHint = document.querySelector("#annotateHint");
const annotateStream = document.querySelector("#annotateStream");
const relatedStream = document.querySelector("#relatedStream");
const webSearchStream = document.querySelector("#webSearchStream");
const annotateFooter = document.querySelector("#annotateFooter");
const annotationList = document.querySelector("#annotationList");
const annotationEmpty = document.querySelector("#annotationEmpty");
const annotateComposer = document.querySelector("#annotateComposer");
const annotationDraftQuote = document.querySelector("#annotationDraftQuote");
const annotationDraft = document.querySelector("#annotationDraft");
const newAnnotationButton = document.querySelector("#newAnnotation");
const addAnnotationButton = document.querySelector("#addAnnotation");
const cancelAnnotationButton = document.querySelector("#cancelAnnotation");
const selectionActions = document.querySelector("#selectionActions");
const selectionAnnotateButton = document.querySelector("#selectionAnnotate");
const selectionAgentButton = document.querySelector("#selectionAgent");
const selectionSearchButton = document.querySelector("#selectionSearch");
const selectionRelatedButton = document.querySelector("#selectionRelated");
const annotateSaveState = document.querySelector("#annotateSaveState");
const annotateFoot = document.querySelector("#annotateFoot");
const AnnotationCodec = window.StillwriteAnnotations;
const DocumentLinks = window.StillwriteDocumentLinks;
const AgentEvents = window.StillwriteAgentEvents;
const Feeds = window.StillwriteFeeds;
const WorkView = window.StillwriteWorkView;
const agentAskDialog = document.querySelector("#agentAskDialog");
const agentAskForm = document.querySelector("#agentAskForm");
const agentAskTitle = document.querySelector("#agentAskTitle");
const agentAskContext = document.querySelector("#agentAskContext");
const agentAskQuote = document.querySelector("#agentAskQuote");
const agentAskRefs = document.querySelector("#agentAskRefs");
const agentAskRefList = document.querySelector("#agentAskRefList");
const agentAskPrompt = document.querySelector("#agentAskPrompt");
const agentAskCancel = document.querySelector("#agentAskCancel");
const agentAskSend = document.querySelector("#agentAskSend");
const settingsButton = document.querySelector("#settingsButton");
const settingsDialog = document.querySelector("#settingsDialog");
const settingsForm = document.querySelector("#settingsForm");
const braveApiKeyInput = document.querySelector("#braveApiKey");
const braveApiKeyStatus = document.querySelector("#braveApiKeyStatus");
const clearBraveApiKeyButton = document.querySelector("#clearBraveApiKey");
const cancelSettingsButton = document.querySelector("#cancelSettings");
const saveSettingsButton = document.querySelector("#saveSettings");

let rootPath = localStorage.getItem("stillwrite.rootPath");
let currentFile = null;
let currentLibraryDocument = null;
let currentAgentDocument = null;
let libraryMode = false;
let workMode = false;
// Work 视图状态：works 表数据（真实 M1 数据，不用 mock）、当前选中的 Work、
// 「旧 Agent 工作」兼容子视图、Work 中栏 Surface 是否可见、5s 轮询。
let workItems = [];
let currentWork = null;
let legacyAgentView = false;
let workSurfaceActive = false;
let workPollTimer = null;
let workEvidenceToken = 0;
// 当前 Work 的证据缓存：work.* 事件（倒序）+ receipt 存在性探针结果。
let workEventCache = [];
let receiptProbe = null;
let librarySources = [];
let libraryStatsData = { total_documents: 0, unique_documents: 0 };
let feedSources = [];
let feedRecent = [];
let rssLibrarySource = null;
let feedLastRefreshAt = null;
let feedBusy = false;
const citationBasket = new Map();
let agentWorkItems = [];
let pendingAgentRequest = null;
let localAgentRuns = [];
// 持久化运行收据里的失败记录（AppData/agent/runs），重启应用后仍可见。
let historicAgentFailures = [];
let saveTimer = null;
let sidebarWidth = Number(
	localStorage.getItem("stillwrite.sidebarWidth") || 248,
);
let splitRatio = Number(localStorage.getItem("stillwrite.splitRatio") || 50);
let sidebarVisible =
	localStorage.getItem("stillwrite.sidebarVisible") !== "false";
let viewMode = localStorage.getItem("stillwrite.viewMode") || "split";
// 写|双|读 document-local（M3）：每个文档记住自己的视图模式，
// localStorage 只存展示偏好（AGENTS §2.1 允许），默认回退 split。
const viewModesByDocument = (() => {
	try {
		const parsed = JSON.parse(
			localStorage.getItem("stillwrite.viewModes") || "{}",
		);
		return parsed && typeof parsed === "object" ? parsed : {};
	} catch (_) {
		return {};
	}
})();
let loadToken = 0;

let annotateVisible =
	localStorage.getItem("stillwrite.annotateVisible") === "true";
let annotateWidth = Number(
	localStorage.getItem("stillwrite.annotateWidth") || 360,
);
let annotationItems = []; // 当前文档的结构化批注
let annotateLoadedDoc = null; // 当前加载了批注的 DocumentRef key
let annotationLoadToken = 0; // 丢弃切换文档时迟到的批注读取结果
let annotateDirty = false;
let annotateTimer = null;
let activeAnnotationId = null;
let pendingAnnotation = null;
let pendingPreviewSelection = null;
let supportView = "annotation";
let relatedItems = [];
let relatedTimer = null;
let relatedRequestToken = 0;
let relatedSeedFingerprint = "";
let relatedHasSearched = false;
let relatedSupplementSeeds = [];
let webSearchHistory = [];
const expandedWebSearchIds = new Set();
const linkedWebSearchResultIds = new Set();
let webSearchMessage = "";
const RELATED_PIN_STORAGE_KEY = "stillwrite.relatedPinned.v1";
const RELATED_PIN_SCOPE_SEPARATOR = "\u001f";
// 固定关联的 durable 事实在 state.db（relations + events）；
// 这个 Map 只是当前会话的运行时缓存，由 list_related_pins 水合。
const relatedPinnedItems = new Map();
let relatedPinsHydratedScope = null;

const DEFAULT_REMOTE = "user@example.invalid:~/stillwrite.git";
let autoSync = false; // 首次手动同步成功后开启自动同步
let syncTimer = null;
let searchTimer = null;
let webSearchRequestToken = 0;
let webSearchHistoryToken = 0;
let previewTimer = null;
let lastTreeNodes = [];
let documentLinkIndex = DocumentLinks.buildIndex([], rootPath);
let agentBusy = false;
let agentProbed = false;
let agentCancelRequested = false;
let activeAgentRunId = null;
let agentEventUnlisten = null;
let agentEventListenerPromise = null;
const agentRunWaiters = new Map();

const ALLOWED_PREVIEW_TAGS = new Set([
	"P",
	"H1",
	"H2",
	"H3",
	"H4",
	"H5",
	"H6",
	"STRONG",
	"EM",
	"DEL",
	"CODE",
	"PRE",
	"UL",
	"OL",
	"LI",
	"BLOCKQUOTE",
	"A",
	"HR",
	"DIV",
	"SPAN",
	"BR",
	"IMG",
	"TABLE",
	"THEAD",
	"TBODY",
	"TR",
	"TH",
	"TD",
	"INPUT",
]);

function escapeHtml(value) {
	return value
		.replaceAll("&", "&amp;")
		.replaceAll("<", "&lt;")
		.replaceAll(">", "&gt;")
		.replaceAll('"', "&quot;")
		.replaceAll("'", "&#039;");
}

function markReadonly() {
	saveStateEl.textContent = "只读资料";
	saveStateEl.classList.remove("dirty", "error");
}

function currentDocumentRef() {
	if (currentFile) return { kind: "workspace", path: currentFile };
	if (currentLibraryDocument) {
		return {
			kind: "library",
			sourceId: currentLibraryDocument.source_id,
			relativePath: currentLibraryDocument.relative_path,
			contentHash: currentLibraryDocument.content_hash,
		};
	}
	if (currentAgentDocument)
		return { kind: "agent", id: currentAgentDocument.id };
	return null;
}

function documentRefKey(ref = currentDocumentRef()) {
	if (!ref) return null;
	if (ref.kind === "workspace") return `workspace:${ref.path}`;
	if (ref.kind === "library")
		return `library:${ref.sourceId}:${ref.relativePath}:${ref.contentHash}`;
	return `agent:${ref.id}`;
}

function currentDocumentUri() {
	const ref = currentDocumentRef();
	if (!ref) return null;
	if (ref.kind === "workspace")
		return `workspace://${relativeDocumentPath(ref.path).replaceAll("\\", "/")}`;
	if (ref.kind === "library") return currentLibraryDocument?.uri || null;
	return currentAgentDocument?.uri || `agent://${ref.id}`;
}

function libraryRefFor(hit, document = hit) {
	return {
		kind: "library",
		sourceId: document.source_id,
		relativePath: document.relative_path,
		contentHash: document.content_hash,
	};
}

function isWorkspaceDocumentForRelated() {
	return Boolean(
		currentFile &&
			!currentLibraryDocument &&
			!currentAgentDocument &&
			!libraryMode &&
			!workMode,
	);
}

function relatedText(value) {
	return String(value || "")
		.replace(/\s+/g, " ")
		.trim();
}

function relatedKey(value) {
	return relatedText(value).normalize("NFKC").toLowerCase();
}

function relatedPath(value) {
	return String(value || "")
		.replaceAll("\\", "/")
		.replace(/\/+/g, "/")
		.replace(/\/\.\//g, "/")
		.replace(/\/$/, "");
}

function relatedScopeStoragePrefix() {
	return `${relatedPinScope()}${RELATED_PIN_SCOPE_SEPARATOR}`;
}

function validStoredRelatedItem(item) {
	return (
		Boolean(item) &&
		typeof item === "object" &&
		typeof item.key === "string" &&
		["annotation", "library", "workspace"].includes(item.kind)
	);
}

// durable URI 与卡片 key 的转换。UI key 保留旧格式（library:library://…），
// 但 Relation target 必须使用规范的 library://… / workspace://… URI。
function relatedItemTargetUri(item) {
	if (item?.kind === "library") {
		const uri = item.raw?.uri;
		if (typeof uri === "string" && uri.includes("://")) return uri;
	}
	if (item?.kind === "workspace" || item?.kind === "annotation") {
		const relative = relatedWorkspaceRelativePath(
			item.path || item.raw?.path || "",
		);
		if (relative) return `workspace://${relative.replace(/^\/+/, "")}`;
	}
	const key = String(item?.key || "");
	const separator = key.indexOf(":");
	if (separator > 0) {
		const scheme = key.slice(0, separator);
		const subject = key.slice(separator + 1);
		if (subject.startsWith(`${scheme}://`)) return subject;
		return `${scheme}://${subject.replace(/^\/+/, "")}`;
	}
	return key;
}

function relatedKeyFromTargetUri(uri) {
	const separator = uri.indexOf("://");
	if (separator < 0) return uri;
	const scheme = uri.slice(0, separator);
	const subject = uri.slice(separator + 3);
	// 兼容早期 P2a 试运行时误写入的 library://library://… URI。
	if (scheme === "library" && subject.startsWith("library://"))
		return `library:${subject}`;
	return scheme === "library" ? `library:${uri}` : `${scheme}:${subject}`;
}

/// 把 legacy localStorage 固定项一次性导入 state.db：
/// 仅迁移当前工作区 scope 的条目并从旧值中移除；其他工作区的条目留待
/// 切换过去时再迁。导入后回读 DB 投影校验，成功才落盘剩余部分。
async function migrateLegacyRelatedPins() {
	const raw = localStorage.getItem(RELATED_PIN_STORAGE_KEY);
	if (!raw) return;
	let parsed;
	try {
		parsed = JSON.parse(raw);
	} catch (error) {
		console.warn("解析 legacy 关联固定项失败，保留原值等待重试", error);
		return;
	}
	if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
		console.warn("legacy 关联固定项格式无效，保留原值等待重试");
		return;
	}
	const prefix = relatedScopeStoragePrefix();
	const matched = [];
	const leftover = {};
	for (const [storageKey, item] of Object.entries(parsed)) {
		if (storageKey.startsWith(prefix) && validStoredRelatedItem(item))
			matched.push(item);
		else leftover[storageKey] = item;
	}
	if (!matched.length) {
		if (!Object.keys(leftover).length)
			localStorage.removeItem(RELATED_PIN_STORAGE_KEY);
		else if (JSON.stringify(leftover) !== JSON.stringify(parsed))
			localStorage.setItem(RELATED_PIN_STORAGE_KEY, JSON.stringify(leftover));
		return;
	}
	await invoke("import_related_pins", {
		items: matched.map((item) => ({
			targetUri: relatedItemTargetUri(item),
			snapshot: serializableRelatedItem(item),
		})),
	});
	// 校验：每个迁移项都必须能在 DB 投影中找到，否则保留旧值下次重试
	const views = await invoke("list_related_pins");
	const pinnedUris = new Set(views.map((view) => view.relation.target_uri));
	if (!matched.every((item) => pinnedUris.has(relatedItemTargetUri(item))))
		throw new Error("关联固定项迁移校验未通过");
	if (Object.keys(leftover).length)
		localStorage.setItem(RELATED_PIN_STORAGE_KEY, JSON.stringify(leftover));
	else localStorage.removeItem(RELATED_PIN_STORAGE_KEY);
}

/// 以 state.db 为准水合当前工作区的固定关联缓存。
/// 在 useWorkspace 内 await：保证任何相关面板渲染前缓存已就绪。
async function hydrateRelatedPins() {
	if (!rootPath || relatedPinsHydratedScope === rootPath) return;
	relatedPinsHydratedScope = rootPath;
	let migrationFailed = false;
	try {
		await migrateLegacyRelatedPins();
	} catch (error) {
		migrationFailed = true;
		relatedPinsHydratedScope = null;
		console.warn("legacy 关联固定项迁移失败，保留旧数据下次重试", error);
	}
	try {
		const views = await invoke("list_related_pins");
		const prefix = relatedScopeStoragePrefix();
		for (const storageKey of [...relatedPinnedItems.keys()]) {
			if (storageKey.startsWith(prefix)) relatedPinnedItems.delete(storageKey);
		}
		for (const view of views) {
			const snapshot =
				view.snapshot && typeof view.snapshot === "object"
					? { ...view.snapshot }
					: {};
			snapshot.key = relatedKeyFromTargetUri(view.relation.target_uri);
			const restored = restoreRelatedPinnedItem(snapshot);
			if (restored)
				relatedPinnedItems.set(`${prefix}${snapshot.key}`, restored);
		}
		renderRelatedPanel();
		if (migrationFailed) relatedPinsHydratedScope = null;
	} catch (error) {
		relatedPinsHydratedScope = null; // 允许下次重试
		console.warn("读取固定关联失败", error);
	}
}

/// ★/☆ 切换后的持久化写入：与旧的 Map 写入语义一致，只是事实源换成 state.db。
async function persistRelatedPinChange(item, pinned, storageKey, previous) {
	try {
		if (pinned)
			await invoke("pin_related", {
				targetUri: relatedItemTargetUri(item),
				snapshot: serializableRelatedItem(item),
			});
		else
			await invoke("unpin_related", { targetUri: relatedItemTargetUri(item) });
	} catch (error) {
		console.warn("同步关联固定状态失败", error);
		// 后端写入失败时撤销乐观 UI 更新；若用户已经再次点击，则保留最新意图。
		if (relatedPinnedItems.has(storageKey) !== pinned) return;
		if (pinned) relatedPinnedItems.delete(storageKey);
		else if (previous) relatedPinnedItems.set(storageKey, previous);
		item.pinned = !pinned;
		relatedItems = [...relatedItems].sort(
			(a, b) => Number(b.pinned) - Number(a.pinned),
		);
		renderRelatedPanel();
	}
}

function relatedPinScope() {
	return relatedPath(rootPath) || "__no_workspace__";
}

function relatedPinStorageKey(item) {
	return `${relatedPinScope()}${RELATED_PIN_SCOPE_SEPARATOR}${item.key}`;
}

function isRelatedPinned(item) {
	return relatedPinnedItems.has(relatedPinStorageKey(item));
}

function serializableRelatedItem(item) {
	return {
		key: item.key,
		kind: item.kind,
		category: item.category,
		title: item.title,
		snippet: item.snippet,
		source: item.source,
		path: item.path || null,
		raw: item.raw || {},
	};
}

function restoreRelatedPinnedItem(item) {
	if (!item || typeof item.key !== "string") return null;
	return {
		key: item.key,
		kind: ["annotation", "library", "workspace"].includes(item.kind)
			? item.kind
			: "workspace",
		category: item.category || "关联",
		title: item.title || item.key,
		snippet: item.snippet || "",
		source: item.source || "",
		path: item.path || null,
		raw: item.raw && typeof item.raw === "object" ? item.raw : {},
		pinned: true,
		score: 0,
		keywordKey: "",
		matchedQueries: new Set(),
		titleMatches: 0,
		phraseMatches: 0,
		order: 0,
	};
}

function currentPinnedRelatedItems() {
	const prefix = `${relatedPinScope()}${RELATED_PIN_SCOPE_SEPARATOR}`;
	return [...relatedPinnedItems.entries()]
		.filter(([storageKey]) => storageKey.startsWith(prefix))
		.map(([, item]) => restoreRelatedPinnedItem(item))
		.filter(Boolean);
}

function toggleRelatedPinned(item) {
	const storageKey = relatedPinStorageKey(item);
	const pinned = !relatedPinnedItems.has(storageKey);
	const previous = relatedPinnedItems.get(storageKey);
	if (pinned) relatedPinnedItems.set(storageKey, serializableRelatedItem(item));
	else relatedPinnedItems.delete(storageKey);
	void persistRelatedPinChange(item, pinned, storageKey, previous);
	item.pinned = pinned;
	relatedItems = [...relatedItems].sort(
		(a, b) => Number(b.pinned) - Number(a.pinned),
	);
	renderRelatedPanel();
	if (!pinned) {
		relatedSeedFingerprint = "";
		scheduleRelatedRefresh(0);
	}
}

function stripRelatedMarkdown(value) {
	return relatedText(value)
		.replace(/\[([^\]]+)\]\([^)]*\)/g, "$1")
		.replace(/[`*_~]/g, "")
		.replace(/^\s*#{1,6}\s+/, "")
		.replace(/^\s*>\s?/, "")
		.trim();
}

function boundRelatedText(value, length = 100) {
	const text = stripRelatedMarkdown(value);
	return text.length > length ? `${text.slice(0, length)}…` : text;
}

const RELATED_STOPWORDS = new Set([
	"a",
	"an",
	"and",
	"are",
	"as",
	"at",
	"be",
	"but",
	"by",
	"can",
	"do",
	"does",
	"for",
	"from",
	"how",
	"in",
	"into",
	"is",
	"it",
	"of",
	"on",
	"or",
	"that",
	"the",
	"this",
	"to",
	"was",
	"what",
	"when",
	"where",
	"why",
	"with",
	"we",
	"you",
	"为什么",
	"如何",
	"怎么",
	"真正",
	"改变的",
	"不是",
	"而是",
	"这个",
	"那个",
	"关于",
	"一种",
	"我们",
	"你们",
	"他们",
	"以及",
	"还是",
	"到底",
	"来自",
	"用于",
	"可以",
	"能够",
	"正在",
	"成为",
	"通过",
	"已经",
	"没有",
	"所有",
	"其中",
	"应该",
	"当前",
	"回到",
	"中",
	"的",
	"了",
	"着",
	"是",
	"在",
	"与",
	"和",
	"及",
	"或",
	"但",
	"也",
	"就",
	"都",
	"被",
	"让",
	"把",
	"给",
	"从",
	"向",
	"对",
	"将",
	"而",
	"又",
	"很",
	"更",
	"最",
	"有",
	"无",
]);

const RELATED_CJK_BOUNDARIES =
	/为什么|如何|怎么|真正|改变的|不是|而是|这个|那个|关于|一种|我们|你们|他们|以及|还是|到底|来自|用于|可以|能够|正在|成为|通过|已经|没有|所有|其中|应该|当前|回到|中|的|了|着|是|在|与|和|及|或|但|也|就|都|被|让|把|给|从|向|对|将|而|又|很|更|最|有|无/g;

function isGenericRelatedFilename(value) {
	return /^(?:untitled|new|document|markdown|note|notes|未命名|无标题|新建文档)(?:[-_ ]*\d+)?$/i.test(
		relatedText(value),
	);
}

function isRelatedKeyword(value) {
	const key = relatedKey(value);
	return Boolean(
		key &&
			!RELATED_STOPWORDS.has(key) &&
			!/^[a-z]$/i.test(key) &&
			!/^[\u3400-\u4dbf\u4e00-\u9fff]$/.test(key),
	);
}

function addRelatedKeyword(map, query, source, weight, options = {}) {
	const clean = relatedText(query);
	const key = relatedKey(clean);
	if (!isRelatedKeyword(clean)) return;
	const existing = map.get(key);
	if (existing) {
		existing.isPhrase ||= Boolean(options.isPhrase);
		existing.priority = Math.max(existing.priority, options.priority || 0);
		return;
	}
	map.set(key, {
		query: clean,
		weight,
		source,
		isPhrase: Boolean(options.isPhrase),
		priority: options.priority || 0,
		order: map.size,
	});
}

function relatedCjkSegments(run) {
	return run
		.split(RELATED_CJK_BOUNDARIES)
		.map((part) => part.trim())
		.filter((part) => part.length >= 2);
}

function extractRelatedKeywordCandidates(value, source, weight) {
	const map = new Map();
	const text = stripRelatedMarkdown(value);
	const tokenPattern =
		/[A-Za-z][A-Za-z0-9+#._-]*|[\u3400-\u4dbf\u4e00-\u9fff]+/g;
	for (const match of text.matchAll(tokenPattern)) {
		const token = match[0];
		if (/^[A-Za-z]/.test(token)) {
			addRelatedKeyword(map, token, source, weight, {
				priority: 96,
			});
			continue;
		}
		for (const segment of relatedCjkSegments(token)) {
			if (segment.length <= 8) {
				addRelatedKeyword(map, segment, source, weight, {
					isPhrase: segment.length >= 3,
					priority: 100 + segment.length,
				});
			} else {
				for (
					let index = 0;
					index + 3 <= segment.length && index < 12;
					index += 2
				) {
					addRelatedKeyword(
						map,
						segment.slice(index, index + 3),
						source,
						weight,
						{
							isPhrase: true,
							priority: 92 - index,
						},
					);
				}
			}
			if (segment.length >= 4) {
				const prefixLength = segment.length >= 5 ? 3 : 2;
				addRelatedKeyword(map, segment.slice(0, prefixLength), source, weight, {
					isPhrase: prefixLength >= 3,
					priority: 88,
				});
				addRelatedKeyword(map, segment.slice(-2), source, weight, {
					priority: 87,
				});
			}
		}
	}
	return [...map.values()].sort(
		(a, b) => b.priority - a.priority || a.order - b.order,
	);
}

function firstRelatedParagraph(source) {
	for (const block of source.split(/\n\s*\n/)) {
		const paragraph = block
			.split("\n")
			.filter((line) => !/^\s*#{1,6}\s+/.test(line))
			.join(" ");
		const opening = boundRelatedText(paragraph);
		if (opening) return opening;
	}
	return "";
}

function relatedSeedCandidates() {
	if (!isWorkspaceDocumentForRelated()) return [];
	const source = editor.value.replace(/\r\n?/g, "\n");
	const h1Line = source.split("\n").find((line) => /^\s{0,3}#\s+/.test(line));
	const h1 = h1Line ? h1Line.replace(/^\s{0,3}#\s+/, "") : "";
	// 占位标题（Untitled / 未命名 等）与文件名一样不产生检索意图。
	const h1Candidates =
		h1 && !isGenericRelatedFilename(h1)
			? extractRelatedKeywordCandidates(h1, "h1", 1)
			: [];
	const filename = basename(currentFile).replace(/\.(md|markdown)$/i, "");
	const filenameCandidates =
		filename && !isGenericRelatedFilename(filename)
			? extractRelatedKeywordCandidates(filename, "filename", 0.6)
			: [];
	const titleCandidates = h1Candidates.length
		? h1Candidates
		: filenameCandidates;
	const supplementCandidates = relatedSupplementSeeds.flatMap((text, index) =>
		extractRelatedKeywordCandidates(text, "selection", 1.2).map(
			(candidate) => ({
				...candidate,
				priority: candidate.priority + 18 - index,
			}),
		),
	);
	const candidates = [
		...supplementCandidates.slice(0, 4),
		...titleCandidates.slice(0, 5),
		...(h1Candidates.length
			? filenameCandidates.slice(0, 2)
			: filenameCandidates.slice(5, 7)),
	];

	if (titleCandidates.length < 2) {
		candidates.push(
			...extractRelatedKeywordCandidates(
				firstRelatedParagraph(source),
				"paragraph",
				0.35,
			),
		);
	}

	const english = h1Candidates.find((candidate) =>
		/^[A-Za-z]/.test(candidate.query),
	);
	const cjkPhrase = h1Candidates
		.filter(
			(candidate) =>
				/^[\u3400-\u4dbf\u4e00-\u9fff]/.test(candidate.query) &&
				candidate.query.length >= 3,
		)
		.sort((a, b) => a.query.length - b.query.length)[0];
	if (english && cjkPhrase) {
		candidates.unshift({
			query: `${english.query} ${cjkPhrase.query}`,
			weight: 1,
			source: "h1",
			isPhrase: true,
			priority: 120,
			order: -1,
		});
	}

	const seen = new Set();
	return candidates
		.filter((candidate) => {
			const key = relatedKey(candidate.query);
			if (!key || seen.has(key)) return false;
			seen.add(key);
			return true;
		})
		.slice(0, 6);
}

function relatedFingerprint(candidates) {
	return candidates
		.map((candidate) => relatedKey(candidate.query))
		.join("\u001f");
}

function renderRelatedToolbar() {
	const count = relatedItems.length;
	relatedCount.textContent = String(count);
}

function renderWebSearchToolbar() {
	webSearchCount.textContent = String(webSearchHistory.length);
}

function setSupportView(view) {
	supportView = [
		"related",
		"web-search",
		"work-context",
		"work-evidence",
	].includes(view)
		? view
		: "annotation";
	const related = supportView === "related";
	const webSearch = supportView === "web-search";
	const workContext = supportView === "work-context";
	const workEvidence = supportView === "work-evidence";
	const documentView = !related && !webSearch && !workContext && !workEvidence;
	annotationViewButton.classList.toggle("active", documentView);
	relatedViewButton.classList.toggle("active", related);
	webSearchViewButton.classList.toggle("active", webSearch);
	workContextViewButton.classList.toggle("active", workContext);
	workEvidenceViewButton.classList.toggle("active", workEvidence);
	annotationViewButton.setAttribute("aria-selected", String(documentView));
	relatedViewButton.setAttribute("aria-selected", String(related));
	webSearchViewButton.setAttribute("aria-selected", String(webSearch));
	workContextViewButton.setAttribute("aria-selected", String(workContext));
	workEvidenceViewButton.setAttribute("aria-selected", String(workEvidence));
	newAnnotationButton.hidden = documentView ? false : true;
	annotateStream.hidden = !documentView;
	annotateFooter.hidden = !documentView;
	relatedStream.hidden = !related;
	webSearchStream.hidden = !webSearch;
	workContextStream.hidden = !workContext;
	workEvidenceStream.hidden = !workEvidence;
	if (related) renderRelatedPanel();
	if (webSearch) renderWebSearchState();
	if (workContext) renderWorkContextPanel();
	if (workEvidence) renderWorkEvidencePanel();
}

function renderRelatedPanel() {
	relatedStream.replaceChildren();
	if (relatedSupplementSeeds.length) {
		const note = document.createElement("div");
		note.className = "related-supplement-note";
		note.textContent = `已加入 ${relatedSupplementSeeds.length} 个选区作为关联线索`;
		relatedStream.appendChild(note);
	}
	if (!relatedItems.length) {
		const tip = document.createElement("div");
		tip.className = "related-empty";
		tip.textContent = relatedHasSearched
			? "暂时没有找到明显相关的旧材料。"
			: "写下标题或开头后，这里会出现与你当前作品相关的旧材料。";
		relatedStream.appendChild(tip);
		return;
	}

	const fragment = document.createDocumentFragment();
	for (const item of relatedItems) {
		const card = document.createElement("article");
		card.className = `related-card${item.pinned ? " pinned" : ""}`;
		const open = document.createElement("button");
		open.type = "button";
		open.className = "related-card-open";
		const category = document.createElement("span");
		category.className = "related-card-category";
		category.textContent = item.category;
		const title = document.createElement("strong");
		title.className = "related-card-title";
		title.textContent = item.title;
		const snippet = document.createElement("span");
		snippet.className = "related-card-snippet";
		snippet.textContent = item.snippet || item.source;
		const source = document.createElement("span");
		source.className = "related-card-source";
		source.textContent = item.source;
		open.append(category, title, snippet, source);
		open.addEventListener("click", () => void openRelatedItem(item));
		card.appendChild(open);

		const actions = document.createElement("div");
		actions.className = "related-card-actions";
		if (item.kind === "library") {
			const citation = document.createElement("label");
			citation.className = "related-card-citation";
			const checkbox = document.createElement("input");
			checkbox.type = "checkbox";
			checkbox.checked = citationBasket.has(item.raw.uri);
			checkbox.title = "加入当前引用";
			checkbox.addEventListener("change", () => {
				if (checkbox.checked) citationBasket.set(item.raw.uri, item.raw);
				else citationBasket.delete(item.raw.uri);
				renderCitationSummary();
			});
			citation.append(checkbox, document.createTextNode("引用"));
			actions.appendChild(citation);
		}

		const pin = document.createElement("button");
		pin.type = "button";
		pin.className = "related-card-pin";
		pin.setAttribute("aria-pressed", String(Boolean(item.pinned)));
		pin.title = item.pinned
			? "取消固定这条关联"
			: "固定这条关联，不受检索刷新影响";
		pin.textContent = item.pinned ? "★ 已固定" : "☆ 固定";
		pin.addEventListener("click", (event) => {
			event.stopPropagation();
			toggleRelatedPinned(item);
		});
		actions.appendChild(pin);
		card.appendChild(actions);
		fragment.appendChild(card);
	}
	relatedStream.appendChild(fragment);
}

function clearRelated({ resetView = true } = {}) {
	if (relatedTimer) {
		clearTimeout(relatedTimer);
		relatedTimer = null;
	}
	hideSelectionAnnotate();
	relatedRequestToken += 1;
	relatedItems = [];
	relatedSeedFingerprint = "";
	relatedHasSearched = false;
	relatedSupplementSeeds = [];
	renderRelatedToolbar();
	if (resetView) setSupportView("annotation");
	else renderRelatedPanel();
}

function addRelatedSupplement(value) {
	if (!isWorkspaceDocumentForRelated()) return;
	const clean = boundRelatedText(value, 240);
	if (!clean) return;
	const key = relatedKey(clean);
	if (relatedTimer) {
		clearTimeout(relatedTimer);
		relatedTimer = null;
	}
	relatedRequestToken += 1;
	relatedSupplementSeeds = [
		clean,
		...relatedSupplementSeeds.filter((seed) => relatedKey(seed) !== key),
	].slice(0, 3);
	relatedItems = [];
	relatedSeedFingerprint = "";
	relatedHasSearched = false;
	renderRelatedToolbar();
	setAnnotateVisible(true);
	setSupportView("related");
	scheduleRelatedRefresh(0);
}

function scheduleRelatedRefresh(delay = 1200) {
	if (relatedTimer) clearTimeout(relatedTimer);
	relatedRequestToken += 1;
	const token = relatedRequestToken;
	relatedTimer = setTimeout(() => {
		relatedTimer = null;
		if (token === relatedRequestToken) void refreshRelated();
	}, delay);
}

async function searchRelatedPlane(command, candidate) {
	try {
		const hits = await invoke(command, { query: candidate.query, limit: 8 });
		return Array.isArray(hits) ? hits : [];
	} catch (error) {
		console.warn("关联搜索失败", command, candidate.query, error);
		return [];
	}
}

function relatedWorkspaceRelativePath(path) {
	return relatedPath(relativeDocumentPath(path));
}

function isExcludedRelatedWorkspacePath(path) {
	const normalized = relatedPath(path);
	const relative = relatedWorkspaceRelativePath(path);
	const current = relatedPath(currentFile);
	const currentRelative = relatedWorkspaceRelativePath(currentFile);
	const currentAnnotation = currentRelative.startsWith("批注/")
		? ""
		: relatedPath(`${rootPath}/批注/${currentRelative}`);
	// 索引进来的 hit.path 是相对路径，而 currentFile 是绝对路径：
	// 两种坐标系都参与比较，任何一侧命中都排除。
	return (
		normalized === current ||
		normalized === currentAnnotation ||
		relative === currentRelative ||
		relative === relatedPath(`批注/${currentRelative}`) ||
		relative === "批注汇总.md" ||
		relative.endsWith("/批注汇总.md")
	);
}

function relatedTitleMatches(hit, candidate) {
	const title = relatedKey(hit.title);
	const terms = relatedKey(candidate.query).split(/\s+/).filter(Boolean);
	return terms.length > 0 && terms.every((term) => title.includes(term));
}

function relatedEvidence(hit, candidate, rank) {
	const titleMatch = relatedTitleMatches(hit, candidate);
	return {
		keywordKey: relatedKey(candidate.query),
		titleMatch,
		score:
			candidate.weight +
			(titleMatch ? 2 : 0) +
			(candidate.isPhrase ? 2 : 0) +
			0.1 / (rank + 1),
	};
}

function normalizeRelatedWorkspaceHit(hit, candidate, rank) {
	if (!hit?.path || isExcludedRelatedWorkspacePath(hit.path)) return null;
	const relative = relatedWorkspaceRelativePath(hit.path);
	const annotation = relative === "批注" || relative.startsWith("批注/");
	const evidence = relatedEvidence(hit, candidate, rank);
	return {
		key: `workspace:${relatedPath(hit.path)}`,
		kind: annotation ? "annotation" : "workspace",
		category: annotation ? "过去的批注" : "工作区",
		title: hit.title || basename(hit.path),
		snippet: relatedText(hit.snippet),
		source: `${annotation ? "批注" : "工作区"} · ${relative}`,
		path: hit.path,
		raw: hit,
		score: evidence.score,
		keywordKey: evidence.keywordKey,
		matchedQueries: new Set([evidence.keywordKey]),
		titleMatches: evidence.titleMatch ? 1 : 0,
		phraseMatches: candidate.isPhrase ? 1 : 0,
	};
}

function normalizeRelatedLibraryHit(hit, candidate, rank) {
	if (!hit?.uri) return null;
	const evidence = relatedEvidence(hit, candidate, rank);
	return {
		key: `library:${hit.uri}`,
		kind: "library",
		category: "资料",
		title: hit.title || hit.relative_path || hit.uri,
		snippet: relatedText(hit.snippet),
		source: `${hit.source_name || "资料"} · ${hit.relative_path || hit.uri}`,
		raw: hit,
		score: evidence.score,
		keywordKey: evidence.keywordKey,
		matchedQueries: new Set([evidence.keywordKey]),
		titleMatches: evidence.titleMatch ? 1 : 0,
		phraseMatches: candidate.isPhrase ? 1 : 0,
	};
}

function mergeRelatedHits(batches) {
	const merged = new Map();
	let order = 0;
	for (const batch of batches) {
		batch.hits.forEach((hit, rank) => {
			const item =
				batch.plane === "workspace"
					? normalizeRelatedWorkspaceHit(hit, batch.candidate, rank)
					: normalizeRelatedLibraryHit(hit, batch.candidate, rank);
			if (!item) return;
			item.pinned = isRelatedPinned(item);
			const existing = merged.get(item.key);
			if (existing) {
				if (!existing.matchedQueries.has(item.keywordKey)) {
					existing.score += item.score;
					existing.matchedQueries.add(item.keywordKey);
					existing.titleMatches += item.titleMatches;
					existing.phraseMatches += item.phraseMatches;
				}
			} else {
				item.order = order;
				order += 1;
				merged.set(item.key, item);
			}
		});
	}
	for (const pinned of currentPinnedRelatedItems()) {
		const existing = merged.get(pinned.key);
		if (existing) {
			existing.pinned = true;
			continue;
		}
		pinned.order = order;
		order += 1;
		merged.set(pinned.key, pinned);
	}

	const categoryOrder = { annotation: 0, workspace: 1, library: 2 };
	const sorted = [...merged.values()].sort(
		(a, b) =>
			Number(b.pinned) - Number(a.pinned) ||
			b.score - a.score ||
			b.matchedQueries.size - a.matchedQueries.size ||
			b.phraseMatches - a.phraseMatches ||
			b.titleMatches - a.titleMatches ||
			categoryOrder[a.kind] - categoryOrder[b.kind] ||
			a.title.localeCompare(b.title, "zh-CN") ||
			a.source.localeCompare(b.source, "zh-CN") ||
			a.order - b.order,
	);
	const pinned = sorted.filter((item) => item.pinned);
	const unpinned = sorted.filter((item) => !item.pinned).slice(0, 5);
	return [...pinned, ...unpinned];
}

async function refreshRelated() {
	const token = relatedRequestToken;
	if (!isWorkspaceDocumentForRelated()) return;
	const candidates = relatedSeedCandidates();
	const fingerprint = relatedFingerprint(candidates);
	if (fingerprint === relatedSeedFingerprint) return;
	relatedSeedFingerprint = fingerprint;
	if (!candidates.length) {
		relatedItems = mergeRelatedHits([]);
		relatedHasSearched = false;
		renderRelatedToolbar();
		renderRelatedPanel();
		return;
	}

	const batches = await Promise.all(
		candidates.flatMap((candidate) => [
			searchRelatedPlane("search_related_index", candidate).then((hits) => ({
				candidate,
				plane: "workspace",
				hits,
			})),
			searchRelatedPlane("search_related_library", candidate).then((hits) => ({
				candidate,
				plane: "library",
				hits,
			})),
		]),
	);
	if (token !== relatedRequestToken || !isWorkspaceDocumentForRelated()) return;
	relatedItems = mergeRelatedHits(batches);
	relatedHasSearched = true;
	renderRelatedToolbar();
	renderRelatedPanel();
}

function workspacePathExists(path) {
	const target = relatedPath(path);
	function visit(nodes) {
		for (const node of nodes || []) {
			if (node.is_dir && visit(node.children)) return true;
			if (!node.is_dir && relatedPath(node.path) === target) return true;
		}
		return false;
	}
	return visit(lastTreeNodes);
}

function annotationSourcePath(sidecarPath) {
	if (!rootPath) return null;
	const relative = relatedWorkspaceRelativePath(sidecarPath);
	if (!relative.startsWith("批注/")) return null;
	const sourceRelative = relative.slice("批注/".length);
	if (!sourceRelative || sourceRelative === "批注汇总.md") return null;
	const root = relatedPath(rootPath);
	const sourcePath = `${root}/${sourceRelative}`;
	return workspacePathExists(sourcePath) ? sourcePath : null;
}

function workspacePathForRelated(path) {
	const value = String(path || "");
	if (!value || !rootPath) return value;
	const normalized = value.replaceAll("\\", "/");
	const root = rootPath.replaceAll("\\", "/").replace(/\/$/, "");
	if (
		normalized === root ||
		normalized.startsWith(`${root}/`) ||
		/^\/?[A-Za-z]:\//.test(normalized) ||
		normalized.startsWith("/")
	)
		return value;
	return `${root}/${normalized.replace(/^\/+/, "")}`;
}

async function openRelatedItem(item) {
	if (item.kind === "library") {
		await openLibraryDocument(item.raw);
		return;
	}
	if (item.kind === "annotation") {
		const sourcePath = annotationSourcePath(item.path) || item.path;
		const openPath = workspacePathForRelated(sourcePath);
		await openFile(openPath, basename(openPath), null);
		setSupportView("annotation");
		setAnnotateVisible(true);
		return;
	}
	const openPath = workspacePathForRelated(item.path);
	await openFile(openPath, basename(openPath), null);
}

function renderCitationSummary() {
	const count = citationBasket.size;
	worksetCount.textContent = `当前引用 · ${count} 篇`;
	worksetBar.hidden = count === 0;
}

async function loadCitationContext() {
	if (!citationBasket.size) return "";
	const sections = [];
	let remaining = 60000;
	for (const hit of citationBasket.values()) {
		if (remaining <= 0) break;
		try {
			const document = await invoke("read_library_document", {
				sourceId: hit.source_id,
				relativePath: hit.relative_path,
			});
			const content = document.content.slice(0, Math.min(12000, remaining));
			remaining -= content.length;
			sections.push(
				`### ${document.title}\n来源：${document.source_name} · ${document.relative_path}\n引用：${document.uri}\n\n${content}`,
			);
		} catch (error) {
			sections.push(
				`### ${hit.title}\n引用：${hit.uri}\n（读取失败：${String(error)}）`,
			);
		}
	}
	return [
		"以下是 StillWrite 当前引用的只读资料。它们来自 Library，不属于 Workspace；回答涉及事实时请优先引用这些资料。",
		sections.join("\n\n---\n\n"),
		"以上是当前引用。",
	].join("\n\n");
}

function waitForAgentRun(runId) {
	return new Promise((resolve, reject) => {
		agentRunWaiters.set(runId, { resolve, reject });
	});
}

function rejectAgentRunWaiter(runId, error) {
	const waiter = agentRunWaiters.get(runId);
	if (!waiter) return;
	agentRunWaiters.delete(runId);
	waiter.reject(error);
}

function handleAgentEvent(envelope) {
	const payload =
		envelope?.payload && typeof envelope.payload === "object"
			? envelope.payload
			: envelope;
	const runId = payload?.runId;
	if (!runId) return;
	const run = localAgentRuns.find((item) => item.id === runId);
	if (!run || !AgentEvents) return;
	Object.assign(run, AgentEvents.applyAgentEvent(run, payload));
	if (workMode && legacyAgentView) renderAgentWorks();
	if (AgentEvents.TERMINAL_EVENTS.has(payload.type)) {
		const waiter = agentRunWaiters.get(runId);
		if (waiter) {
			agentRunWaiters.delete(runId);
			waiter.resolve(payload);
		}
	}
}

async function installAgentEventListener() {
	if (agentEventUnlisten) return;
	if (!agentEventListenerPromise) {
		agentEventListenerPromise = (async () => {
			if (typeof listen !== "function") {
				throw new Error("StillWrite 无法订阅 Pi Agent 事件");
			}
			agentEventUnlisten = await listen("agent-event", handleAgentEvent);
		})();
		agentEventListenerPromise.catch(() => {
			agentEventListenerPromise = null;
		});
	}
	await agentEventListenerPromise;
}

async function probeAgent() {
	if (agentProbed) return;
	await invoke("agent_probe");
	agentProbed = true;
}

function formatAgentTime(value) {
	if (!value) return "刚刚";
	const millis = value < 100000000000 ? value * 1000 : value;
	return new Date(millis).toLocaleString("zh-CN", {
		month: "numeric",
		day: "numeric",
		hour: "2-digit",
		minute: "2-digit",
	});
}

function agentWorkTitle(prompt) {
	const firstLine =
		prompt
			.split(/\r?\n/)
			.map((line) => line.trim())
			.find(Boolean) || "Agent 工作";
	const clean = firstLine.replace(/^#+\s*/, "").replace(/[。！？.!?]+$/, "");
	return clean.length > 56 ? `${clean.slice(0, 56)}…` : clean;
}

function agentDocumentContent(text, title) {
	const body = String(text || "").trim();
	if (!body) return `# ${title}\n\nAgent 没有返回正文。\n`;
	return /^#\s+/.test(body) ? `${body}\n` : `# ${title}\n\n${body}\n`;
}

function makeLocalAgentRun(request, prompt) {
	const id = `local-${globalThis.crypto?.randomUUID?.() || `${Date.now()}-${Math.random().toString(36).slice(2)}`}`;
	return {
		id,
		title: agentWorkTitle(prompt),
		prompt,
		originUri: request?.originUri || null,
		originQuote: request?.originQuote || null,
		updatedAt: Math.floor(Date.now() / 1000),
		status: "running",
		streamText: "",
		preview: "",
		toolStatus: "",
		terminal: false,
		local: true,
	};
}

function allAgentWorkItems() {
	return [...localAgentRuns, ...agentWorkItems].sort(
		(a, b) => (b.updatedAt || 0) - (a.updatedAt || 0),
	);
}

function buildAgentPrompt(request, prompt, citationContext) {
	const source = request?.originUri || "（当前工作没有明确来源文档）";
	const selected = request?.originQuote
		? `> ${request.originQuote.replaceAll("\n", "\n> ")}`
		: "（没有选中文本）";
	return [
		`# Current source\n${source}`,
		`# Selected text\n${selected}`,
		citationContext
			? `# Explicit references\n${citationContext}`
			: "# Explicit references\n（没有显式引用资料）",
		`# User request\n${prompt}`,
	].join("\n\n");
}

async function cancelAgentTurn() {
	if (!agentBusy || !activeAgentRunId) return false;
	agentCancelRequested = true;
	const run = localAgentRuns.find((item) => item.id === activeAgentRunId);
	if (run) {
		run.status = "停止中";
		run.updatedAt = Math.floor(Date.now() / 1000);
		renderAgentWorks();
	}
	try {
		const response = await invoke("agent_abort");
		// The command also emits this event from Rust.  Applying the local
		// acknowledgement makes a workspace switch safe even if the WebView
		// receives that event after the switch has returned.
		handleAgentEvent({ type: "agent_stopped", runId: activeAgentRunId });
		return Boolean(response?.accepted);
	} catch (error) {
		console.error("停止 Agent 失败", error);
		handleAgentEvent({
			type: "error",
			runId: activeAgentRunId,
			message: String(error),
		});
		return false;
	}
}

function renderInline(text) {
	let value = escapeHtml(text);
	const code = [];
	value = value.replace(/`([^`]+)`/g, (_, body) => {
		const token = `@@SWC${code.length}_${Math.random().toString(36).slice(2)}@@`;
		code.push({ token, html: `<code>${body}</code>` });
		return token;
	});
	// 图片：先于链接处理，避免 `![alt](src)` 被链接规则吞掉。
	// 远程 https/http → 直接 src；本地相对路径 → data-local-src 后置水合。
	value = value.replace(/!\[([^\]]*)\]\(([^)\s]+)\)/g, (_, alt, src) => {
		const raw = src.replaceAll("&amp;", "&");
		if (/^https?:\/\//i.test(raw)) {
			return `<img src="${raw}" alt="${alt || ""}" loading="lazy">`;
		}
		return `<img data-local-src="${raw}" alt="${alt || ""}" loading="lazy">`;
	});
	value = value.replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>");
	value = value.replace(/__([^_]+)__/g, "<strong>$1</strong>");
	value = value.replace(/(^|[^*])\*([^*\n]+)\*/g, "$1<em>$2</em>");
	value = value.replace(/~~([^~]+)~~/g, "<del>$1</del>");
	value = value.replace(
		/\[([^\]]+)\]\((?:&lt;([^>]+)&gt;|([^)\s]+))\)/g,
		(_, label, angleHref, plainHref) => {
			const href = angleHref || plainHref;
			const raw = href.replaceAll("&amp;", "&");
			const safe = /^\s*(?:javascript|data):/i.test(raw) ? "#" : href;
			const target = /^https?:/i.test(raw)
				? ' target="_blank" rel="noreferrer"'
				: "";
			return `<a href="${safe}"${target}>${label}</a>`;
		},
	);
	code.forEach(({ token, html }) => {
		value = value.replaceAll(token, () => html);
	});
	return value;
}

function splitTableRow(line) {
	// 首尾可选 `|`；`\|` 视为正文 pipe，不拆列。
	const cells = [];
	let current = "";
	const body = line.trim();
	const start = body.startsWith("|") ? 1 : 0;
	const end = body.endsWith("|") ? body.length - 1 : body.length;
	for (let i = start; i < end; i += 1) {
		const ch = body[i];
		if (ch === "\\") {
			// 转义序列整体保留：`\|` 是正文 pipe，不触发分列
			current += ch;
			if (i + 1 < end) {
				current += body[i + 1];
				i += 1;
			}
			continue;
		}
		if (ch === "|") {
			cells.push(current);
			current = "";
			continue;
		}
		current += ch;
	}
	cells.push(current);
	return cells.map((cell) => cell.trim());
}

function isTableDelimiter(line) {
	const cells = splitTableRow(line);
	return (
		cells.length >= 2 &&
		cells.every((cell) => /^:?-{3,}:?$/.test(cell.replace(/^\s+|\s+$/g, "")))
	);
}

function tableCellAlignClass(delimiter) {
	const value = delimiter.replace(/^\s+|\s+$/g, "");
	if (value.startsWith(":") && value.endsWith(":")) return "align-center";
	if (value.endsWith(":")) return "align-right";
	if (value.startsWith(":")) return "align-left";
	return "align-left";
}

function renderTable(headerCells, delimiterCells, rows) {
	const align = headerCells.map((_, i) =>
		tableCellAlignClass(delimiterCells[i] || "---"),
	);
	const thead = `<thead><tr>${headerCells
		.map(
			(cell, i) =>
				`<th class="${align[i]}">${renderInline(cell)}</th>`,
		)
		.join("")}</tr></thead>`;
	const tbody = rows.length
		? `<tbody>${rows
				.map(
					(row) =>
						`<tr>${row
							.map(
								(cell, i) =>
									`<td class="${align[i] || "align-left"}">${renderInline(cell)}</td>`,
							)
							.join("")}</tr>`,
				)
				.join("")}</tbody>`
		: "";
	return `<table>${thead}${tbody}</table>`;
}

function renderMarkdown(source) {
	const lines = source.replace(/\r\n?/g, "\n").split("\n");
	const html = [];
	let paragraph = [];
	let listType = null;
	let inCode = false;
	let codeLang = "";
	let codeLines = [];

	const flushParagraph = () => {
		if (!paragraph.length) return;
		// 段落内软换行保留为 <br>：多行笔记（如批注正文）不会在阅读区被并成一行
		html.push(
			`<p>${paragraph.map((line) => renderInline(line)).join("<br>")}</p>`,
		);
		paragraph = [];
	};
	const closeList = () => {
		if (!listType) return;
		html.push(`</${listType}>`);
		listType = null;
	};
	const flushCode = () => {
		const cls = codeLang ? ` class="language-${escapeHtml(codeLang)}"` : "";
		html.push(
			`<pre><code${cls}>${escapeHtml(codeLines.join("\n"))}</code></pre>`,
		);
		codeLang = "";
		codeLines = [];
	};

	for (let i = 0; i < lines.length; i += 1) {
		const line = lines[i];
		// 批注侧车中的结构标记是 Markdown 注释，阅读时不应显示。
		if (
			/^<!-- \/?stillwrite-(?:annotations|annotation|quote)/.test(line.trim())
		)
			continue;
		if (
			currentFile &&
			!isAnnotatablePath(currentFile) &&
			/^>\s*原文（(?:字句|段落|全文)）：\s*$/.test(line)
		)
			continue;
		const fence = line.match(/^```\s*([^\s]*)\s*$/);
		if (fence) {
			flushParagraph();
			closeList();
			if (inCode) {
				flushCode();
				inCode = false;
			} else {
				inCode = true;
				codeLang = fence[1] || "";
			}
			continue;
		}
		if (inCode) {
			codeLines.push(line);
			continue;
		}

		if (!line.trim()) {
			flushParagraph();
			closeList();
			continue;
		}

		const heading = line.match(/^(#{1,6})\s+(.+)$/);
		if (heading) {
			flushParagraph();
			closeList();
			const level = heading[1].length;
			html.push(`<h${level}>${renderInline(heading[2])}</h${level}>`);
			continue;
		}

		if (/^\s*([-*_])(?:\s*\1){2,}\s*$/.test(line)) {
			flushParagraph();
			closeList();
			html.push("<hr>");
			continue;
		}

		const quote = line.match(/^>\s?(.*)$/);
		if (quote) {
			flushParagraph();
			closeList();
			html.push(`<blockquote><p>${renderInline(quote[1])}</p></blockquote>`);
			continue;
		}

		// GFM 表格：当前行含 `|` 且下一行是合法分隔行
		if (line.includes("|") && lines[i + 1] && isTableDelimiter(lines[i + 1])) {
			const headerCells = splitTableRow(line);
			const delimiterCells = splitTableRow(lines[i + 1]);
			i += 1; // 消费分隔行
			const rows = [];
			while (i + 1 < lines.length) {
				const next = lines[i + 1];
				if (!next.trim() || !next.includes("|")) break;
				if (/^\s*([-*_])(?:\s*\1){2,}\s*$/.test(next)) break;
				i += 1;
				rows.push(splitTableRow(next));
			}
			flushParagraph();
			closeList();
			html.push(renderTable(headerCells, delimiterCells, rows));
			continue;
		}

		const task = line.match(/^\s*[-+*]\s+\[([ xX])\]\s+(.+)$/);
		const ul = line.match(/^\s*[-+*]\s+(.+)$/);
		const ol = line.match(/^\s*\d+[.)]\s+(.+)$/);
		if (task || ul || ol) {
			flushParagraph();
			const wanted = task || ul ? "ul" : "ol";
			if (listType !== wanted) {
				closeList();
				html.push(wanted === "ul" ? '<ul class="task-list">' : "<ol>");
				listType = wanted;
			}
			if (task) {
				const checked = task[1].toLowerCase() === "x";
				html.push(
					`<li class="task-list-item"><input type="checkbox" disabled${checked ? " checked" : ""}>${renderInline(task[2])}</li>`,
				);
			} else {
				html.push(`<li>${renderInline((ul || ol)[1])}</li>`);
			}
			continue;
		}

		paragraph.push(line.trim());
	}

	if (inCode) flushCode();
	flushParagraph();
	closeList();
	return html.join("\n");
}

function sanitizeHtml(html) {
	// Defense-in-depth: renderMarkdown escapes user content, but never let raw
	// script/event markup through if a future renderer regression slips.
	const doc = new DOMParser().parseFromString(html, "text/html");
	[...doc.body.querySelectorAll("*")].reverse().forEach((el) => {
		if (!ALLOWED_PREVIEW_TAGS.has(el.tagName)) {
			el.replaceWith(document.createTextNode(el.textContent));
			return;
		}
		[...el.attributes].forEach((attr) => {
			const name = attr.name.toLowerCase();
			if (name.startsWith("on")) {
				el.removeAttribute(attr.name);
				return;
			}
			// tag-aware attribute allow-list：不再给所有 tag 共用一套属性。
			const tag = el.tagName;
			if (tag === "IMG") {
				const allowed =
					name === "src" ||
					name === "alt" ||
					name === "title" ||
					name === "loading" ||
					name === "class" ||
					name === "data-local-src";
				if (allowed && name === "src" && !/^https?:\/\//i.test(attr.value)) {
					// Markdown 原文中的 data:/javascript: 图片不直接信任
					el.removeAttribute(attr.name);
					return;
				}
				if (!allowed) el.removeAttribute(attr.name);
				return;
			}
			if (tag === "INPUT") {
				if (
					name === "type" &&
					attr.value.toLowerCase() !== "checkbox"
				) {
					el.removeAttribute(attr.name);
					return;
				}
				if (
					name === "checked" ||
					name === "disabled" ||
					name === "class"
				) {
					return; // 保留
				}
				el.removeAttribute(attr.name);
				return;
			}
			if (tag === "TABLE" || tag === "TH" || tag === "TD") {
				if (name !== "class") el.removeAttribute(attr.name);
				return;
			}
			if (name === "href" && /^\s*javascript:/i.test(attr.value)) {
				attr.value = "#";
				return;
			}
			if (name === "href" && /^\s*data:/i.test(attr.value)) {
				attr.value = "#";
				return;
			}
			if (
				name !== "href" &&
				name !== "class" &&
				name !== "target" &&
				name !== "rel"
			)
				el.removeAttribute(attr.name);
		});
	});
	return doc;
}

function decorateExplicitDocumentLinks(root) {
	root.querySelectorAll("a[href]").forEach((anchor) => {
		const doc = DocumentLinks.resolveInternalHref(
			anchor.getAttribute("href"),
			currentFile,
			rootPath,
			documentLinkIndex,
		);
		if (!doc) return;
		anchor.href = "#";
		anchor.dataset.documentPath = doc.path;
		anchor.classList.add("workspace-document-link");
		anchor.title = `打开 ${doc.relative}`;
		anchor.removeAttribute("target");
	});
}

function linkifyPreviewText(root) {
	const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, {
		acceptNode(node) {
			return node.parentElement?.closest("a, code, pre, button")
				? NodeFilter.FILTER_REJECT
				: NodeFilter.FILTER_ACCEPT;
		},
	});
	const nodes = [];
	let node;
	while ((node = walker.nextNode())) nodes.push(node);
	nodes.forEach((textNode) => {
		const segments = DocumentLinks.segmentText(
			textNode.data,
			documentLinkIndex,
		);
		if (!segments.some((segment) => segment.type)) return;
		const fragment = document.createDocumentFragment();
		segments.forEach((segment) => {
			if (!segment.type) {
				fragment.appendChild(document.createTextNode(segment.text));
				return;
			}
			const anchor = document.createElement("a");
			anchor.textContent = segment.label;
			if (segment.type === "external") {
				anchor.href = segment.href;
				anchor.target = "_blank";
				anchor.rel = "noreferrer";
			} else {
				anchor.href = "#";
				anchor.dataset.documentPath = segment.href;
				anchor.className = "workspace-document-link";
				anchor.title = `打开 ${segment.label}`;
			}
			fragment.appendChild(anchor);
		});
		textNode.replaceWith(fragment);
	});
}

function preparePreviewLinks(root) {
	decorateExplicitDocumentLinks(root);
	linkifyPreviewText(root);
}

function previewImageCacheKey(markdownPath) {
	return `${currentFile || ""}\u001f${markdownPath}`;
}

async function hydrateLocalPreviewImages(root) {
	if (!currentFile) return;
	const images = [...root.querySelectorAll("img[data-local-src]")];
	if (!images.length) return;
	const documentPath = currentFile;

	// 同一次 render 内按 key 去重，避免同一图片重复 IPC
	const pending = [];
	const seen = new Set();
	for (const img of images) {
		const markdownPath = img.dataset.localSrc;
		const cacheKey = previewImageCacheKey(markdownPath);
		if (seen.has(cacheKey)) continue;
		seen.add(cacheKey);
		const cached = localImagePreviewCache.get(cacheKey);
		if (cached) {
			img.src = cached;
			continue;
		}
		pending.push({ img, markdownPath, cacheKey });
	}
	if (!pending.length) return;

	// 并发上限 4
	let next = 0;
	const worker = async () => {
		while (next < pending.length) {
			const job = pending[next];
			next += 1;
			try {
				const dataUrl = await invoke("read_workspace_image_data_url", {
					documentPath,
					markdownPath: job.markdownPath,
				});
				localImagePreviewCache.set(job.cacheKey, dataUrl);
				job.img.src = dataUrl;
			} catch (error) {
				console.warn("本地图片预览失败", job.markdownPath, error);
			}
		}
	};
	await Promise.all([worker(), worker(), worker(), worker()]);
}

function clearLocalImagePreviewCache() {
	localImagePreviewCache.clear();
}

/// 恢复欢迎页（updatePreview 会 replaceChildren，无法直接保留初始 DOM）。
function showWelcomePreview() {
	previewEl.replaceChildren();
	const welcome = document.createElement("div");
	welcome.className = "welcome";
	const title = document.createElement("h1");
	title.textContent = "Stillwrite";
	const hint = document.createElement("p");
	hint.textContent = "从“文件”菜单打开文件夹或 Markdown 文档开始写作。";
	const keys = document.createElement("p");
	const kbd1 = document.createElement("kbd");
	kbd1.textContent = "Ctrl+1";
	const kbd2 = document.createElement("kbd");
	kbd2.textContent = "Ctrl+2";
	const kbd3 = document.createElement("kbd");
	kbd3.textContent = "Ctrl+3";
	keys.append(kbd1, " 写作 · ", kbd2, " 双栏 · ", kbd3, " 阅读");
	welcome.append(title, hint, keys);
	previewEl.appendChild(welcome);
}

/// 提取 Tauri 命令错误里人可读的部分；extract 失败时退回 fallback。
function backendErrorMessage(error, fallback) {
	const text = String(error ?? "").replace(
		/^Error invoking remote method '[^']+': Error: /,
		"",
	);
	const trimmed = text.trim();
	if (!trimmed) return fallback;
	return trimmed.length > 120 ? `${trimmed.slice(0, 120)}…` : trimmed;
}

function updatePreview() {
	if (previewTimer) {
		clearTimeout(previewTimer);
		previewTimer = null;
	}
	const doc = sanitizeHtml(renderMarkdown(editor.value));
	preparePreviewLinks(doc.body);
	previewEl.replaceChildren(...doc.body.childNodes);
	renderAnnotationAnchors();
	// 注意：节点已被 replaceChildren 移进 previewEl，必须对 previewEl 水合
	// （doc.body 此刻已为空，传它会导致本地图片永远拿不到 data URL）。
	void hydrateLocalPreviewImages(previewEl);
}

function schedulePreview() {
	if (previewTimer) clearTimeout(previewTimer);
	previewTimer = setTimeout(updatePreview, 100);
}

function markDirty() {
	saveStateEl.textContent = "编辑中…";
	saveStateEl.classList.add("dirty");
	saveStateEl.classList.remove("error");
}

function markSaved() {
	saveStateEl.textContent = "已保存";
	saveStateEl.classList.remove("dirty", "error");
}

function markError(message) {
	saveStateEl.textContent = message;
	saveStateEl.classList.add("error");
}

function scheduleSave() {
	if (!currentFile && !currentAgentDocument) return;
	if (saveTimer) clearTimeout(saveTimer);
	saveTimer = setTimeout(saveCurrent, 650);
}

async function saveCurrent() {
	if (!currentFile && !currentAgentDocument) return;
	if (saveTimer) {
		clearTimeout(saveTimer);
		saveTimer = null;
	}
	try {
		if (currentAgentDocument) {
			currentAgentDocument = await invoke("write_agent_work", {
				input: { id: currentAgentDocument.id, content: editor.value },
			});
		} else {
			await invoke("write_markdown", {
				path: currentFile,
				content: editor.value,
			});
		}
		markSaved();
		if (currentFile && autoSync) scheduleAutoSync();
		if (workMode && legacyAgentView) await loadAgentWorks();
	} catch (error) {
		console.error(error);
		markError("保存失败");
	}
}

function updateLibrarySummary(result) {
	librarySources = result.sources || [];
	libraryStatsData = {
		total_documents: result.total_documents || 0,
		unique_documents: result.unique_documents || 0,
	};
	const sourceCount = librarySources.length;
	libraryStats.textContent = sourceCount
		? `${sourceCount} 个来源 · ${libraryStatsData.total_documents.toLocaleString()} 篇 · 去重后 ${libraryStatsData.unique_documents.toLocaleString()} 篇`
		: "尚未添加资料源";
	if (result.warnings?.length) libraryStats.title = result.warnings.join("\n");
	else libraryStats.removeAttribute("title");
	renderCitationSummary();
}

function renderLibraryHome() {
	treeEl.replaceChildren();
	const fragment = document.createDocumentFragment();

	// 文章才是一级对象：最近 RSS 直接置顶，进资料页先看到内容。
	if (rssLibrarySource || feedSources.length) {
		treeEl.appendChild(renderRssRecentSection());
	}

	const section = document.createElement("div");
	section.className = "library-section";
	const title = document.createElement("div");
	title.className = "library-section-title";
	title.textContent = "来源";
	section.appendChild(title);

	const regularSources = rssLibrarySource
		? librarySources.filter((source) => source.id !== rssLibrarySource.id)
		: librarySources;
	if (!regularSources.length && !feedSources.length) {
		const tip = document.createElement("div");
		tip.className = "empty-tip";
		tip.textContent =
			"还没有资料源。点击右上角 ＋ 添加外部 Markdown 目录、RSS，或导入 OPML。";
		section.appendChild(tip);
	} else {
		for (const source of regularSources) {
			const row = document.createElement("div");
			row.className = `library-source${source.available ? "" : " unavailable"}`;
			row.title = source.root;
			const name = document.createElement("div");
			name.className = "library-source-name";
			const label = document.createElement("span");
			label.textContent = source.name;
			const count = document.createElement("span");
			count.className = "library-source-count";
			count.textContent = source.available
				? `${source.documents.toLocaleString()} 篇`
				: "目录不可用";
			name.append(label, count);
			const root = document.createElement("div");
			root.className = "library-source-root";
			root.textContent = source.root;
			row.append(name, root);
			fragment.appendChild(row);
		}
		if (rssLibrarySource || feedSources.length) {
			fragment.appendChild(renderFeedManagerSection());
		}
	}
	section.appendChild(fragment);
	treeEl.appendChild(section);
}

/// 最近 RSS：一级消费入口，展示最新物化的文章卡片。
function renderRssRecentSection() {
	const wrap = document.createElement("div");
	wrap.className = "rss-section rss-recent";

	const head = document.createElement("div");
	head.className = "rss-recent-head";
	const name = document.createElement("div");
	name.className = "library-source-name";
	const label = document.createElement("span");
	label.className = "recent-title";
	label.textContent = "最近 RSS";
	const count = document.createElement("span");
	count.className = "library-source-count";
	count.id = "rssSourceCount";
	count.textContent = Feeds.rssSourceCountText(feedSources, rssLibrarySource);
	name.append(label, count);
	const actions = document.createElement("div");
	actions.className = "library-source-actions";
	const refreshAll = document.createElement("button");
	refreshAll.type = "button";
	refreshAll.className = "text-btn";
	refreshAll.textContent = "刷新全部";
	refreshAll.title =
		"后台抓取全部 RSS 源（最多 4 路并发），完成后刷新 Library 索引";
	refreshAll.disabled = feedBusy;
	refreshAll.addEventListener("click", (event) => {
		event.stopPropagation();
		void refreshAllFeeds();
	});
	actions.appendChild(refreshAll);
	head.append(name, actions);
	wrap.appendChild(head);

	const recentList = document.createElement("div");
	recentList.className = "feed-recent-list";
	if (!feedRecent.length) {
		const tip = document.createElement("div");
		tip.className = "empty-tip";
		tip.textContent = "刷新后，最新的 RSS 条目会作为普通资料出现在这里。";
		recentList.appendChild(tip);
	} else {
		for (const item of feedRecent) {
			recentList.appendChild(renderFeedRecentCard(item));
		}
	}
	wrap.appendChild(recentList);
	return wrap;
}

/// 订阅源是设置对象：默认折叠，点开做源级别的增删与刷新。
function renderFeedManagerSection() {
	const wrap = document.createElement("div");

	const row = document.createElement("div");
	row.className = "library-source rss-row";
	row.title = "点击展开 / 收起订阅源列表";
	const name = document.createElement("div");
	name.className = "library-source-name";
	const label = document.createElement("span");
	label.textContent = "订阅源";
	const count = document.createElement("span");
	count.className = "library-source-count";
	count.textContent = `${feedSources.length} 个`;
	name.append(label, count);
	wrap.appendChild(row);

	const listWrap = document.createElement("div");
	listWrap.className = "feed-list-wrap collapsed";
	const feedList = document.createElement("div");
	feedList.className = "feed-list";
	if (!feedSources.length) {
		const tip = document.createElement("div");
		tip.className = "empty-tip";
		tip.textContent = "点击右上角 ＋ → 添加 RSS，或导入 OPML。";
		feedList.appendChild(tip);
	} else {
		for (const source of feedSources) {
			feedList.appendChild(renderFeedSourceRow(source));
		}
	}
	listWrap.appendChild(feedList);
	wrap.appendChild(listWrap);
	row.addEventListener("click", (event) => {
		if (event.target.closest("button")) return;
		listWrap.classList.toggle("collapsed");
	});
	return wrap;
}

function renderFeedSourceRow(source) {
	const rowEl = document.createElement("div");
	rowEl.className = `feed-source-row${source.last_error ? " feed-error" : ""}`;
	const head = document.createElement("div");
	head.className = "feed-source-head";
	const nameEl = document.createElement("span");
	nameEl.className = "feed-source-name";
	nameEl.textContent = source.name;
	nameEl.title = source.url;
	const statusEl = document.createElement("span");
	statusEl.className = "feed-source-status";
	statusEl.textContent = Feeds.feedSourceStatusText(source);
	statusEl.title = source.url;
	head.append(nameEl, statusEl);
	const urlEl = document.createElement("div");
	urlEl.className = "feed-source-url";
	urlEl.textContent = source.url;
	const actions = document.createElement("div");
	actions.className = "feed-source-actions";
	const refresh = document.createElement("button");
	refresh.type = "button";
	refresh.className = "icon-btn subtle";
	refresh.textContent = "↻";
	refresh.title = "刷新此源";
	refresh.addEventListener("click", (event) => {
		event.stopPropagation();
		void refreshFeedSource(source.id);
	});
	const remove = document.createElement("button");
	remove.type = "button";
	remove.className = "icon-btn subtle";
	remove.textContent = "✕";
	remove.title = "删除此源（本地缓存随之删除，批注保留）";
	remove.addEventListener("click", (event) => {
		event.stopPropagation();
		void removeFeedSource(source.id, source.name);
	});
	actions.append(refresh, remove);
	rowEl.append(head, urlEl, actions);
	return rowEl;
}

function renderFeedRecentCard(item) {
	const card = document.createElement("button");
	card.type = "button";
	card.className = "feed-recent-card";
	const meta = document.createElement("span");
	meta.className = "feed-recent-meta";
	const feedName = item.feed_name || "RSS";
	const date = item.date ? item.date.replaceAll("-", ".") : "";
	meta.textContent = date ? `${feedName} · ${date}` : feedName;
	const title = document.createElement("strong");
	title.className = "feed-recent-title";
	title.textContent = item.title || item.relative_path;
	const snippet = document.createElement("span");
	snippet.className = "feed-recent-snippet";
	snippet.textContent = item.snippet || "";
	card.append(meta, title, snippet);
	card.title = item.uri;
	card.addEventListener("click", () => void openLibraryDocument(item));
	return card;
}

function setSourceMenuOpen(open) {
	sourceMenu.hidden = !open;
}

function showFeedMessage(text) {
	feedStatusLine.textContent = text || "";
	feedStatusLine.hidden = !text;
}

async function loadFeedStatus() {
	try {
		const status = await invoke("feed_status");
		feedSources = status.sources || [];
		feedRecent = status.recent || [];
		rssLibrarySource = status.rss_library_source || null;
		feedLastRefreshAt = status.last_refresh_at || null;
		if (libraryMode) renderLibraryHome();
		return status;
	} catch (error) {
		console.error("读取 Feed 状态失败", error);
		return null;
	}
}

function submitAddFeed(event) {
	event.preventDefault();
	const url = addFeedUrl.value.trim();
	if (!url) return;
	addFeedDialog.close();
	addFeedUrl.value = "";
	void (async () => {
		try {
			const view = await invoke("feed_add_source", { url });
			showFeedMessage(`已添加「${view.name}」，后台抓取中…`);
		} catch (error) {
			console.error(error);
			showFeedMessage(String(error));
		}
		await Promise.all([refreshLibrary(), loadFeedStatus()]);
		if (!libraryMode) await setSidebarMode("library");
	})();
}

async function importOpml() {
	try {
		const result = await invoke("feed_import_opml");
		const warning = result.warnings?.length
			? `（${result.warnings[0]}${result.warnings.length > 1 ? ` 等 ${result.warnings.length} 条警告` : ""}）`
			: "";
		showFeedMessage(
			`已添加 ${result.added} 个，${result.duplicates} 个已存在，${result.invalid} 个无效。${warning}`,
		);
		await Promise.all([refreshLibrary(), loadFeedStatus()]);
	} catch (error) {
		if (String(error).includes("未选择")) return;
		console.error(error);
		showFeedMessage(`OPML 导入失败：${String(error)}`);
	}
}

async function refreshAllFeeds() {
	if (feedBusy) return;
	feedBusy = true;
	showFeedMessage("正在刷新全部 Feed…");
	try {
		const result = await invoke("feed_refresh_all");
		const failed = result.failed || 0;
		showFeedMessage(
			`新增 ${result.added} · 更新 ${result.updated} · ${failed} 个源失败`,
		);
		await Promise.all([refreshLibrary(true), loadFeedStatus()]);
	} catch (error) {
		console.error(error);
		showFeedMessage("Feed 刷新失败");
	} finally {
		feedBusy = false;
	}
}

async function refreshFeedSource(id) {
	if (feedBusy) return;
	feedBusy = true;
	try {
		const outcome = await invoke("feed_refresh_source", { id });
		if (outcome.status === "error") {
			showFeedMessage(`${outcome.name} 刷新失败：${outcome.error}`);
		} else {
			showFeedMessage(
				`${outcome.name} · 新增 ${outcome.added} · 更新 ${outcome.updated}` +
					(outcome.status === "unchanged" ? "（无变化）" : ""),
			);
		}
		await Promise.all([refreshLibrary(true), loadFeedStatus()]);
	} catch (error) {
		console.error(error);
		showFeedMessage(`刷新失败：${String(error)}`);
	} finally {
		feedBusy = false;
	}
}

async function removeFeedSource(id, name) {
	if (
		!window.confirm(
			`删除「${name}」？本地缓存的 RSS 资料会一并删除（批注保留）。`,
		)
	)
		return;
	try {
		await invoke("feed_remove_source", { id });
		showFeedMessage(`已删除「${name}」`);
		await Promise.all([refreshLibrary(true), loadFeedStatus()]);
	} catch (error) {
		console.error(error);
		showFeedMessage(`删除失败：${String(error)}`);
	}
}

/// 打开 Library 平面时：超过 30 分钟未全局刷新则后台触发一次（不阻塞 UI）。
function maybeAutoRefreshFeeds() {
	if (feedBusy || !feedSources.length) return;
	const now = Math.floor(Date.now() / 1000);
	if (feedLastRefreshAt && now - feedLastRefreshAt < 30 * 60) return;
	void refreshAllFeeds();
}

function renderLibrarySearchResults(hits) {
	treeEl.replaceChildren();
	if (!hits.length) {
		const tip = document.createElement("div");
		tip.className = "empty-tip";
		tip.textContent = "没有匹配的资料";
		treeEl.appendChild(tip);
		return;
	}
	const fragment = document.createDocumentFragment();
	for (const hit of hits) {
		const row = document.createElement("div");
		row.className = "library-hit";
		const open = document.createElement("button");
		open.type = "button";
		open.className = "tree-file library-hit-open";
		open.title = hit.uri;
		const title = document.createElement("span");
		title.className = "hit-title";
		title.textContent = hit.title;
		const snippet = document.createElement("span");
		snippet.className = "hit-snippet";
		snippet.textContent = `${hit.source_name} · ${hit.relative_path}\n${hit.snippet || ""}`;
		open.append(title, snippet);
		open.addEventListener("click", () => void openLibraryDocument(hit));

		const actions = document.createElement("label");
		actions.className = "library-hit-actions";
		const checkbox = document.createElement("input");
		checkbox.type = "checkbox";
		checkbox.checked = citationBasket.has(hit.uri);
		checkbox.title = "加入当前引用";
		checkbox.addEventListener("change", () => {
			if (checkbox.checked) citationBasket.set(hit.uri, hit);
			else citationBasket.delete(hit.uri);
			renderCitationSummary();
		});
		actions.append(checkbox, document.createTextNode("引用"));
		row.append(open, actions);
		fragment.appendChild(row);
	}
	treeEl.appendChild(fragment);
}

async function refreshLibrary() {
	try {
		const result = await invoke("refresh_library");
		updateLibrarySummary(result);
		if (libraryMode) {
			if (searchInput.value.trim()) await runSearch();
			else renderLibraryHome();
		}
		return result;
	} catch (error) {
		console.error(error);
		markError("资料库刷新失败");
		return null;
	}
}

async function addLibrarySource() {
	try {
		const result = await invoke("add_library_source");
		updateLibrarySummary(result);
		if (!libraryMode) await setSidebarMode("library");
		else if (searchInput.value.trim()) await runSearch();
		else renderLibraryHome();
	} catch (error) {
		if (String(error).includes("未选择")) return;
		console.error(error);
		markError("添加资料源失败");
	}
}

async function setSidebarMode(mode) {
	const nextLibraryMode = mode === "library";
	const nextWorkMode = mode === "work";
	if (libraryMode === nextLibraryMode && workMode === nextWorkMode) {
		if (workMode && !legacyAgentView) await loadWorks();
		return;
	}
	clearWebSearch();
	libraryMode = nextLibraryMode;
	workMode = nextWorkMode;
	legacyAgentView = false;
	currentWork = null;
	workspaceTab.classList.toggle("active", !libraryMode && !workMode);
	libraryTab.classList.toggle("active", libraryMode);
	workTabRef.classList.toggle("active", workMode);
	workspaceTab.setAttribute("aria-selected", String(!libraryMode && !workMode));
	libraryTab.setAttribute("aria-selected", String(libraryMode));
	workTabRef.setAttribute("aria-selected", String(workMode));
	addLibrarySourceButton.hidden = !libraryMode;
	newAgentWorkButton.hidden = !workMode;
	libraryMeta.hidden = !libraryMode;
	workMetaEl.hidden = !workMode;
	legacyAgentWorksToggle.textContent = "旧 Agent 工作";
	searchInput.placeholder = libraryMode
		? "搜索资料库…"
		: workMode
			? "搜索工作…"
			: "搜索全文（FTS）…";
	searchInput.value = "";
	clearTimeout(searchTimer);
	if (libraryMode || workMode) clearRelated();
	else if (isWorkspaceDocumentForRelated()) scheduleRelatedRefresh(0);
	renderCitationSummary();
	stopWorkPolling();
	setWorkSurface(workMode);
	updateFormatToolbarState();
	if (libraryMode) {
		if (!librarySources.length) await refreshLibrary();
		else renderLibraryHome();
		await loadFeedStatus();
		maybeAutoRefreshFeeds();
	} else if (workMode) {
		await loadWorks();
	} else if (lastTreeNodes.length) {
		renderTree(lastTreeNodes);
	} else {
		treeEl.replaceChildren();
	}
}

async function useWorkspace(data) {
	if (!data) return;
	const workspaceChanged = !rootPath || !samePath(rootPath, data.root);
	if (workspaceChanged) {
		for (const [runId, waiter] of agentRunWaiters) {
			waiter.reject(new Error("Workspace 已切换，Agent 工作未完成"));
			agentRunWaiters.delete(runId);
		}
		localAgentRuns = [];
		activeAgentRunId = null;
		agentProbed = false;
		citationBasket.clear();
		renderCitationSummary();
		clearRelated();
		clearWebSearch({ clearHistory: true });
		clearLocalImagePreviewCache();
	}
	editor.readOnly = false;
	editor.classList.remove("readonly");
	editorPaneLabel.textContent = "WRITE";
	annotationButton.disabled = false;
	aggregateButton.disabled = false;
	currentLibraryDocument = null;
	currentAgentDocument = null;
	workSurfaceActive = false;
	loadViewModeForCurrentDocument();
	rootPath = data.root;
	relatedPinsHydratedScope = null; // 换工作区必须重新水合对应的固定关联
	localStorage.setItem("stillwrite.rootPath", rootPath);
	workspaceNameEl.textContent = basename(rootPath);
	workspaceNameEl.title = rootPath;
	lastTreeNodes = data.nodes;
	documentLinkIndex = DocumentLinks.buildIndex(data.nodes, rootPath);
	renderTree(data.nodes);
	await hydrateRelatedPins();
	await loadWebSearchHistory();
	// works 按 workspace_id 隔离：换工作区后重查当前工作区的工作
	if (workMode && !legacyAgentView) await loadWorks();
}

async function chooseWorkspace() {
	try {
		if (agentBusy) await cancelAgentTurn();
		// Save against the current workspace before Rust switches the active root.
		await saveCurrent();
		if (annotateDirty) await saveAnnotate();
		const data = await invoke("choose_workspace");
		if (!data) return;
		await openWorkspaceData(data);
	} catch (error) {
		console.error(error);
		markError("目录打开失败");
	}
}

/// choose_workspace 与 open_recent_workspace 共用的 Workspace 打开投影。
async function openWorkspaceData(data) {
	currentFile = null;
	currentAgentDocument = null;
	editor.value = "";
	updatePreview();
	documentTitleEl.textContent = "Stillwrite";
	await setSidebarMode("workspace");
	await useWorkspace(data);
	loadAnnotationPanel();
	updateFormatToolbarState();
}

async function chooseDocument() {
	try {
		if (agentBusy) await cancelAgentTurn();
		await saveCurrent();
		if (annotateDirty) await saveAnnotate();
		const data = await invoke("choose_document");
		if (!data) return;
		currentFile = data.path;
		currentAgentDocument = null;
		await setSidebarMode("workspace");
		await useWorkspace(data);
		showDocument(data.path, data.name, data.content, null);
	} catch (error) {
		console.error(error);
		markError("文档打开失败");
	}
}

async function restoreWorkspace() {
	if (!rootPath) return;
	try {
		const data = await invoke("set_workspace", { path: rootPath });
		await useWorkspace(data);
	} catch (error) {
		console.warn("Cannot restore workspace", error);
		rootPath = null;
		localStorage.removeItem("stillwrite.rootPath");
		workspaceNameEl.textContent = "未打开文件夹";
	}
}

async function refreshTree() {
	if (libraryMode) {
		await refreshLibrary();
		await loadFeedStatus();
		return;
	}
	if (workMode) {
		if (legacyAgentView) {
			await loadHistoricAgentFailures();
			await loadAgentWorks();
		} else {
			await loadWorks();
		}
		return;
	}
	await refreshWorkspace();
}

/// 工作区子菜单的「刷新工作区」：显式刷新 Workspace 投影，不复用 mode-aware
/// 的 refreshTree。same root 时 backend 不视为切换（不取消 Pi、不清缓存）；
/// 不强制切左栏，不关当前 Library / Work projection，不丢当前文档 buffer。
async function refreshWorkspace() {
	if (!rootPath) return;
	try {
		if (!libraryMode && !workMode) {
			await saveCurrent();
			if (annotateDirty && currentFile) await saveAnnotate();
		}
		const data = await invoke("set_workspace", { path: rootPath });
		if (libraryMode || workMode) {
			// 只更新文件树缓存与链接索引，不打断当前资料/工作 projection
			rootPath = data.root;
			lastTreeNodes = data.nodes;
			documentLinkIndex = DocumentLinks.buildIndex(data.nodes, rootPath);
			workspaceNameEl.textContent = basename(rootPath);
			workspaceNameEl.title = rootPath;
		} else {
			await useWorkspace(data);
			if (currentFile && !workspacePathExists(currentFile)) {
				// 当前文档被外部删除：清理 current doc 并提示
				currentFile = null;
				editor.value = "";
				showWelcomePreview();
				documentTitleEl.textContent = "Stillwrite";
				markError("当前文档已被外部删除");
				loadAnnotationPanel();
				updateFormatToolbarState();
			}
		}
	} catch (error) {
		console.error(error);
		markError("刷新工作区失败");
	}
}

/// 关闭工作区：前端先落盘可编辑内容与待保存批注、中止进行中的 Agent run，
/// 再调用真实 backend lifecycle（关闭 Pi、解除 active root），最后清 UI shell 状态。
/// 不删除任何 Markdown / 批注 / durable 状态。
async function closeWorkspace() {
	if (!rootPath) return;
	try {
		if (agentBusy) await cancelAgentTurn();
		await saveCurrent();
		if (annotateDirty) await saveAnnotate();
		await invoke("close_workspace");

		// ---- UI reset：backend 已解除 active root，这里只投影 ----
		rootPath = null;
		localStorage.removeItem("stillwrite.rootPath");
		currentFile = null;
		currentAgentDocument = null;
		currentLibraryDocument = null;
		lastTreeNodes = [];
		documentLinkIndex = DocumentLinks.buildIndex([], null);
		editor.value = "";
		editor.readOnly = false;
		editor.classList.remove("readonly");
		editorPaneLabel.textContent = "WRITE";
		annotationButton.disabled = false;
		aggregateButton.disabled = true;
		showWelcomePreview();
		documentTitleEl.textContent = "Stillwrite";
		workspaceNameEl.textContent = "未打开文件夹";
		workspaceNameEl.title = "";
		markSaved();
		// 运行时投影清空（durable 的 works / recents / relations 不动）
		agentWorkItems = [];
		localAgentRuns = [];
		activeAgentRunId = null;
		agentProbed = false;
		historicAgentFailures = [];
		workItems = [];
		currentWork = null;
		renderAgentWorks();
		renderWorkList();
		renderWorkCenter();
		citationBasket.clear();
		renderCitationSummary();
		clearRelated();
		clearWebSearch({ clearHistory: true });
		clearLocalImagePreviewCache();
		loadAnnotationPanel();
		// 左栏切到 文件 projection，显示空状态而不是空 Work Home
		await setSidebarMode("workspace");
		treeEl.replaceChildren();
		const tip = document.createElement("div");
		tip.className = "empty-tip";
		tip.textContent = "打开文件夹或 Markdown 开始";
		treeEl.appendChild(tip);
		updateFormatToolbarState();
		updateFileMenuState();
	} catch (error) {
		console.error(error);
		markError(backendErrorMessage(error, "关闭工作区失败"));
	}
}

/// 在文件管理器中显示 Workspace root 或其中某个路径（backend 校验边界）。
async function revealPath(path) {
	try {
		await invoke("reveal_workspace_path", { path });
	} catch (error) {
		console.error(error);
		markError(backendErrorMessage(error, "显示失败"));
	}
}

/// Workspace Markdown 另存为：backend 负责对话框 / 边界校验 / 原子写入；
/// 成功后 currentFile 切到新文件，原文件保留，新文档不继承批注 sidecar。
async function saveDocumentAs() {
	if (!currentFile || currentLibraryDocument || currentAgentDocument) return;
	try {
		// 先落盘旧文档的待保存批注：另存后批注上下文会切到新文档
		if (annotateDirty) await saveAnnotate();
		const data = await invoke("save_markdown_as", {
			currentPath: currentFile,
			content: editor.value,
		});
		if (!data) return; // 用户取消
		currentFile = data.path;
		currentLibraryDocument = null;
		currentAgentDocument = null;
		lastTreeNodes = data.nodes;
		documentLinkIndex = DocumentLinks.buildIndex(data.nodes, data.root);
		renderTree(data.nodes);
		documentTitleEl.textContent = data.name.replace(/\.(md|markdown)$/i, "");
		markSaved();
		loadAnnotationPanel();
		scheduleRelatedRefresh(0);
		void loadWebSearchHistory();
		updateFormatToolbarState();
	} catch (error) {
		console.error(error);
		markError(backendErrorMessage(error, "另存为失败"));
	}
}

function shortAgentError(error) {
	const text = String(error).replace(/\s+/g, " ").trim();
	return text.length > 120 ? `${text.slice(0, 120)}…` : text;
}

async function loadHistoricAgentFailures() {
	try {
		const runs = await invoke("agent_recent_runs");
		historicAgentFailures = (runs || []).filter(
			(receipt) => receipt.outcome === "failed",
		);
	} catch (error) {
		console.error("读取历史失败记录失败", error);
		historicAgentFailures = [];
	}
}

async function openFile(path, name, row) {
	const token = ++loadToken;
	await saveCurrent();
	if (annotateDirty) await saveAnnotate();
	try {
		const text = await invoke("read_markdown", { path });
		if (token !== loadToken) return;
		showDocument(path, name, text, row);
	} catch (error) {
		console.error(error);
		markError("打开失败");
	}
}

async function openLibraryDocument(hit) {
	clearRelated();
	clearWebSearch();
	const token = ++loadToken;
	await saveCurrent();
	if (annotateDirty) await saveAnnotate();
	try {
		const data = await invoke("read_library_document", {
			sourceId: hit.source_id,
			relativePath: hit.relative_path,
		});
		if (token !== loadToken) return;
		currentLibraryDocument = data;
		currentFile = null;
		currentAgentDocument = null;
		editor.readOnly = true;
		editor.classList.add("readonly");
		editorPaneLabel.textContent = "READ ONLY · LIBRARY";
		annotationButton.disabled = false;
		aggregateButton.disabled = true;
		annotationItems = [];
		annotateLoadedDoc = null;
		annotateDirty = false;
		editor.value = data.content;
		updatePreview();
		documentTitleEl.textContent = data.title;
		editor.scrollTop = 0;
		previewEl.scrollTop = 0;
		markReadonly();
		loadViewModeForCurrentDocument();
		await loadAnnotationPanel();
		await loadWebSearchHistory();
		updateFormatToolbarState();
	} catch (error) {
		console.error(error);
		markError("资料打开失败");
	}
}

function showDocument(path, name, text, row) {
	clearRelated();
	clearWebSearch();
	currentFile = path;
	currentLibraryDocument = null;
	currentAgentDocument = null;
	editor.readOnly = false;
	editor.classList.remove("readonly");
	editorPaneLabel.textContent = "WRITE";
	annotationButton.disabled = false;
	aggregateButton.disabled = false;
	editor.value = text;
	updatePreview();
	documentTitleEl.textContent = name.replace(/\.(md|markdown)$/i, "");
	document
		.querySelectorAll(".tree-file.active")
		.forEach((el) => el.classList.remove("active"));
	if (row) row.classList.add("active");
	else if (lastTreeNodes.length) renderTree(lastTreeNodes);
	editor.scrollTop = 0;
	previewEl.scrollTop = 0;
	markSaved();
	loadViewModeForCurrentDocument();
	loadAnnotationPanel();
	scheduleRelatedRefresh(0);
	void loadWebSearchHistory();
	updateFormatToolbarState();
}

// ---------------------------------------------------------------------------
// Work Home（M3 Shell）：三组列表 + Work 详情 Surface。
// 数据全部来自 works 表（list_works / work_events / work_receipt_probe），
// 不用 mock；分组/badge/时间由 StillwriteWorkView 纯函数给出。
// ---------------------------------------------------------------------------

async function loadWorks() {
	if (!rootPath) {
		workItems = [];
		renderWorkList();
		renderWorkCenter();
		return;
	}
	try {
		workItems = await invoke("list_works", { limit: null });
	} catch (error) {
		console.error("读取工作失败", error);
		workItems = [];
	}
	if (currentWork) {
		currentWork =
			workItems.find((item) => item.id === currentWork.id) || currentWork;
	}
	renderWorkList();
	renderWorkCenter();
	if (WorkView.activeCount(workItems) > 0) startWorkPolling();
	else stopWorkPolling();
}

function workListRows() {
	const query = searchInput.value.trim();
	return query
		? workItems.filter((item) => WorkView.rowMatches(item, query))
		: workItems;
}

function renderWorkList() {
	treeEl.replaceChildren();
	const rows = workListRows();
	const groups = WorkView.groupWorks(rows);
	workStatsEl.textContent = rows.length
		? `${WorkView.groupWorks(workItems).reduce((sum, group) => sum + group.items.length, 0)} 个工作 · ${WorkView.activeCount(workItems)} 个进行中`
		: "还没有工作";
	if (!rows.length) {
		const tip = document.createElement("div");
		tip.className = "empty-tip agent-empty-tip";
		tip.textContent = searchInput.value.trim()
			? "没有匹配的工作"
			: "还没有工作。选中文字后点「问 Agent」，或点右上角 ＋ 发起新工作。";
		treeEl.appendChild(tip);
		return;
	}
	const fragment = document.createDocumentFragment();
	for (const group of groups) {
		if (!group.items.length) continue; // 空组不渲染标题
		const head = document.createElement("div");
		head.className = "work-group-head";
		head.textContent = `${group.title} · ${group.items.length}`;
		fragment.appendChild(head);
		for (const record of group.items) {
			fragment.appendChild(renderWorkRow(record));
		}
	}
	treeEl.appendChild(fragment);
}

function renderWorkRow(record) {
	const row = WorkView.rowModel(record);
	const item = document.createElement("div");
	item.className = `agent-work-item work-item work-tone-${row.badge.tone}${currentWork?.id === row.id ? " active" : ""}`;
	const open = document.createElement("button");
	open.type = "button";
	open.className = "agent-work-open work-row-open";
	const head = document.createElement("span");
	head.className = "work-row-head";
	const badge = document.createElement("span");
	badge.className = `work-badge work-badge-${row.badge.tone}`;
	badge.textContent = row.badge.label;
	const title = document.createElement("strong");
	title.className = "agent-work-title";
	title.textContent = row.title;
	const time = document.createElement("span");
	time.className = "work-row-time";
	time.textContent = row.relativeTime;
	head.append(badge, title, time);
	const meta = document.createElement("span");
	meta.className = "agent-work-meta";
	meta.textContent =
		row.summary || (row.nextAction ? `下一步：${row.nextAction}` : "");
	open.append(head, meta);
	open.addEventListener("click", () => void openWorkDetail(record));
	item.appendChild(open);
	if (row.artifact?.id) {
		const artifact = document.createElement("button");
		artifact.type = "button";
		artifact.className = "text-btn work-row-artifact";
		artifact.textContent =
			row.status === "needs_human" ? "成果待验收" : "查看成果";
		artifact.title = row.artifact.uri;
		artifact.addEventListener("click", (event) => {
			event.stopPropagation();
			void openWorkArtifact(row.artifact.id);
		});
		item.appendChild(artifact);
	}
	return item;
}

/** Work Surface 显隐：data-mode="work" 时中栏只显示 Work 面板。 */
function setWorkSurface(active) {
	workSurfaceActive = Boolean(active);
	if (workSurfaceActive) {
		setWorkSupportChrome(true);
		if (supportView !== "work-context" && supportView !== "work-evidence") {
			setSupportView("work-context");
		}
	} else {
		setWorkSupportChrome(false);
		if (supportView === "work-context" || supportView === "work-evidence") {
			setSupportView("annotation");
		}
	}
	renderWorkCenter();
	applyLayout();
}

function renderWorkCenter() {
	if (!workSurfaceActive) {
		workHome.hidden = true;
		workDetail.hidden = true;
		return;
	}
	if (!currentWork) {
		renderWorkHomeCenter();
		return;
	}
	renderWorkDetailView();
}

function renderWorkHomeCenter() {
	workDetail.hidden = true;
	workHome.hidden = false;
	const groups = WorkView.groupWorks(workItems);
	workHome.replaceChildren();
	const title = document.createElement("h2");
	title.className = "work-home-title";
	title.textContent = "工作";
	const subtitle = document.createElement("p");
	subtitle.className = "work-home-subtitle";
	subtitle.textContent = rootPath
		? "今天有什么在推进、什么需要你，一眼可见。"
		: "先从「文件」菜单打开一个文件夹，工作会记录在这里。";
	workHome.append(title, subtitle);
	const counts = document.createElement("div");
	counts.className = "work-home-counts";
	for (const group of groups) {
		const card = document.createElement("div");
		card.className = "work-home-count";
		const number = document.createElement("strong");
		number.textContent = String(group.items.length);
		const label = document.createElement("span");
		label.textContent = group.title;
		card.append(number, label);
		counts.appendChild(card);
	}
	workHome.appendChild(counts);
	if (rootPath) {
		const ask = document.createElement("button");
		ask.type = "button";
		ask.className = "primary-btn work-home-ask";
		ask.textContent = "发起 Agent 工作";
		ask.title = "与选区「问 Agent」相同的入口";
		ask.addEventListener(
			"click",
			() => void beginAgentQuestion(null, { allowEmpty: true }),
		);
		workHome.appendChild(ask);
	}
}

function renderWorkDetailView() {
	workHome.hidden = true;
	const record = currentWork;
	if (!record) return;
	const detail = WorkView.detailModel(record, workEventCache, receiptProbe);
	workDetail.hidden = false;
	workDetail.replaceChildren();

	const back = document.createElement("button");
	back.type = "button";
	back.className = "text-btn work-detail-back";
	back.textContent = "← 返回工作";
	back.addEventListener("click", () => closeWorkDetail());

	const head = document.createElement("div");
	head.className = "work-detail-head";
	const badge = document.createElement("span");
	badge.className = `work-badge work-badge-${detail.status.tone}`;
	badge.textContent = detail.status.label;
	const title = document.createElement("h3");
	title.className = "work-detail-title";
	title.textContent = detail.title;
	head.append(badge, title);

	const actions = document.createElement("div");
	actions.className = "work-detail-actions";
	if (detail.canAccept) {
		const accept = document.createElement("button");
		accept.type = "button";
		accept.className = "primary-btn";
		accept.textContent = "验收完成";
		accept.title = "先查看成果文档，确认后再验收；完成只来自你的明确接受";
		accept.addEventListener("click", () => void acceptWork(record.id));
		actions.appendChild(accept);
	}
	if (detail.canCancel) {
		const cancel = document.createElement("button");
		cancel.type = "button";
		cancel.className = "text-btn";
		cancel.textContent = "取消";
		cancel.title = "运行中的工作会同时停止 Pi 进程";
		cancel.addEventListener("click", () => void cancelWorkAction(record.id));
		actions.appendChild(cancel);
	}

	const body = document.createElement("div");
	body.className = "work-detail-body";
	body.appendChild(workDetailSection("目标", detail.intent || "（未记录）"));
	if (detail.statusLine) {
		const reason = detail.statusLine.reason
			? `（${detail.statusLine.reason}）`
			: "";
		body.appendChild(
			workDetailSection("状态变化", `${detail.statusLine.text}${reason}`),
		);
	}
	body.appendChild(
		workDetailSection(
			"下一步",
			detail.nextAction ||
				(record.status === "needs_human" ? "等待人工验收" : "—"),
		),
	);
	const artifactValue = document.createElement("span");
	if (detail.artifact?.id) {
		const link = document.createElement("button");
		link.type = "button";
		link.className = "text-btn work-artifact-link";
		link.textContent = "打开成果文档";
		link.title = detail.artifact.uri;
		link.addEventListener(
			"click",
			() => void openWorkArtifact(detail.artifact.id),
		);
		artifactValue.appendChild(link);
	} else {
		artifactValue.textContent = "尚无成果";
		artifactValue.className = "work-muted";
	}
	body.appendChild(workDetailSection("成果", null, artifactValue));
	body.appendChild(
		workDetailSection("证据", null, workEvidenceSummaryNode(detail)),
	);

	workDetail.append(back, head, actions, body);
}

function workDetailSection(label, text, node = null) {
	const section = document.createElement("div");
	section.className = "work-detail-section";
	const key = document.createElement("div");
	key.className = "work-detail-key";
	key.textContent = label;
	section.appendChild(key);
	if (node) {
		section.appendChild(node);
	} else {
		const value = document.createElement("div");
		value.className = "work-detail-value";
		value.textContent = text;
		section.appendChild(value);
	}
	return section;
}

function workEvidenceSummaryNode(detail) {
	const box = document.createElement("div");
	const receipt = document.createElement("div");
	receipt.className = "work-detail-value";
	if (!detail.receipt.ref) {
		receipt.textContent = "无运行收据引用";
	} else {
		const marker =
			detail.receipt.exists === null
				? "检查中…"
				: detail.receipt.exists
					? "运行收据在档"
					: "运行收据缺失";
		receipt.textContent = `receipt ${detail.receipt.ref} · ${marker}`;
		receipt.classList.toggle("work-muted", detail.receipt.exists === false);
	}
	box.appendChild(receipt);
	const hint = document.createElement("div");
	hint.className = "work-muted work-evidence-hint";
	hint.textContent = "完整事件见右侧「证据」。";
	box.appendChild(hint);
	return box;
}

function closeWorkDetail() {
	currentWork = null;
	workEventCache = [];
	receiptProbe = null;
	workEvidenceCount.textContent = "0";
	renderWorkList();
	renderWorkCenter();
}

async function openWorkDetail(record) {
	currentWork = record;
	workEventCache = [];
	receiptProbe = null;
	workEvidenceCount.textContent = "0";
	if (!workSurfaceActive) setWorkSurface(true);
	else renderWorkCenter();
	if (!annotateVisible) setAnnotateVisible(true); // 右栏：上下文 | 证据
	renderWorkList();
	await refreshWorkEvidence(record.id);
}

async function openWorkArtifact(agentWorkId) {
	if (!agentWorkId) return;
	// 复用现有 Agent Work Markdown 打开路径（可编辑 Artifact）
	await openAgentWork({ id: agentWorkId });
}

async function refreshWorkEvidence(workId) {
	const token = ++workEvidenceToken;
	const record = currentWork;
	if (!record || record.id !== workId) return;
	const results = await Promise.all([
		invoke("work_events", { workId, limit: null }).catch((error) => {
			console.error("读取工作事件失败", error);
			return [];
		}),
		record.receipt_ref
			? invoke("work_receipt_probe", { receiptRef: record.receipt_ref }).catch(
					() => null,
				)
			: Promise.resolve(null),
	]);
	if (token !== workEvidenceToken || currentWork?.id !== workId) return;
	workEventCache = results[0] || [];
	receiptProbe = results[1];
	workEvidenceCount.textContent = String(workEventCache.length);
	renderWorkContextPanel();
	renderWorkEvidencePanel();
	renderWorkCenter();
}

function renderWorkContextPanel() {
	workContextStream.replaceChildren();
	if (!currentWork) {
		const tip = document.createElement("div");
		tip.className = "empty-tip";
		tip.textContent = "尚未选择工作。";
		workContextStream.appendChild(tip);
		return;
	}
	const detail = WorkView.detailModel(
		currentWork,
		workEventCache,
		receiptProbe,
	);
	const intent = document.createElement("div");
	intent.className = "work-panel-block";
	const intentKey = document.createElement("div");
	intentKey.className = "work-detail-key";
	intentKey.textContent = "目标（人的意图，不被 Agent 改写）";
	const intentValue = document.createElement("div");
	intentValue.className = "work-detail-value work-prewrap";
	intentValue.textContent = detail.intent || "（未记录）";
	intent.append(intentKey, intentValue);
	const meta = document.createElement("div");
	meta.className = "work-panel-block";
	const metaKey = document.createElement("div");
	metaKey.className = "work-detail-key";
	metaKey.textContent = "当前状态";
	const metaValue = document.createElement("div");
	metaValue.className = "work-detail-value";
	metaValue.textContent = `${detail.status.label}${detail.statusLine ? ` · ${detail.statusLine.text}` : ""}${detail.nextAction ? ` · 下一步：${detail.nextAction}` : ""}`;
	meta.append(metaKey, metaValue);
	workContextStream.append(intent, meta);
}

function renderWorkEvidencePanel() {
	workEvidenceStream.replaceChildren();
	const detail = currentWork
		? WorkView.detailModel(currentWork, workEventCache, receiptProbe)
		: null;
	if (!detail) {
		const tip = document.createElement("div");
		tip.className = "empty-tip";
		tip.textContent = "尚未选择工作。";
		workEvidenceStream.appendChild(tip);
		return;
	}
	// 当前 Pi 证据只允许：Artifact、receipt、session ref、runtime settlement/failure、
	// Work events。没有真实 adapter 就不显示 tests/diff/commit。
	const receipt = document.createElement("div");
	receipt.className = "work-panel-block";
	const receiptKey = document.createElement("div");
	receiptKey.className = "work-detail-key";
	receiptKey.textContent = "运行收据";
	const receiptValue = document.createElement("div");
	receiptValue.className = "work-detail-value";
	receiptValue.textContent = detail.receipt.ref
		? `${detail.receipt.ref} · ${detail.receipt.exists === null ? "检查中…" : detail.receipt.exists ? "在档" : "缺失"}`
		: "无运行收据引用";
	receipt.append(receiptKey, receiptValue);
	const events = document.createElement("div");
	events.className = "work-panel-block";
	const eventsKey = document.createElement("div");
	eventsKey.className = "work-detail-key";
	eventsKey.textContent = `工作事件 · ${workEventCache.length}`;
	events.appendChild(eventsKey);
	if (!workEventCache.length) {
		const empty = document.createElement("div");
		empty.className = "work-detail-value work-muted";
		empty.textContent = "暂无事件";
		events.appendChild(empty);
	}
	for (const event of detail.events) {
		const line = document.createElement("div");
		line.className = "work-event-line";
		const action = document.createElement("span");
		action.className = "work-event-action";
		action.textContent = event.action;
		const time = document.createElement("span");
		time.className = "work-event-time";
		time.textContent = event.time;
		line.append(action, time);
		if (event.reason) {
			const reason = document.createElement("div");
			reason.className = "work-event-reason";
			reason.textContent = event.reason;
			line.appendChild(reason);
		}
		events.appendChild(line);
	}
	workEvidenceStream.append(receipt, events);
}

async function acceptWork(workId) {
	try {
		const updated = await invoke("work_accept", { workId });
		await refreshWorksAfterAction(updated);
	} catch (error) {
		console.error(error);
		markError(`接受失败：${shortAgentError(error)}`);
	}
}

async function cancelWorkAction(workId) {
	try {
		const updated = await invoke("work_cancel", { workId });
		await refreshWorksAfterAction(updated);
	} catch (error) {
		console.error(error);
		markError(`取消失败：${shortAgentError(error)}`);
	}
}

async function refreshWorksAfterAction(updated) {
	if (updated && currentWork?.id === updated.id) currentWork = updated;
	await loadWorks();
	if (currentWork) await refreshWorkEvidence(currentWork.id);
	if (workMode && legacyAgentView) renderAgentWorks();
}

function startWorkPolling() {
	if (workPollTimer || !workMode || legacyAgentView) return;
	workPollTimer = setInterval(() => {
		if (!workMode || legacyAgentView || document.hidden) return;
		if (WorkView.activeCount(workItems) === 0) {
			stopWorkPolling();
			return;
		}
		void loadWorks();
	}, 5000);
}

function stopWorkPolling() {
	if (workPollTimer) {
		clearInterval(workPollTimer);
		workPollTimer = null;
	}
}

/** 「旧 Agent 工作」兼容入口：迁移期间旧列表保持可达，不先删后迁。 */
async function toggleLegacyAgentWorks() {
	legacyAgentView = !legacyAgentView;
	legacyAgentWorksToggle.textContent = legacyAgentView
		? "返回工作"
		: "旧 Agent 工作";
	if (legacyAgentView) {
		currentWork = null;
		workEventCache = [];
		receiptProbe = null;
		setWorkSurface(false);
		stopWorkPolling();
		await loadHistoricAgentFailures();
		await loadAgentWorks();
	} else {
		setWorkSurface(true);
		await loadWorks();
	}
}

/** Work Surface 下右栏切换为 上下文|证据；文档批注/关联/搜索入口隐藏。 */
function setWorkSupportChrome(workVisible) {
	const docTabs = [
		annotationViewButton,
		relatedViewButton,
		webSearchViewButton,
	];
	const workTabs = [workContextViewButton, workEvidenceViewButton];
	for (const tab of docTabs) tab.hidden = workVisible;
	for (const tab of workTabs) tab.hidden = !workVisible;
	if (!workVisible) return; // 离开时由 setSupportView 按当前视图恢复
	newAnnotationButton.hidden = true;
	annotateFooter.hidden = true;
	annotateDocName.textContent = currentWork?.title || "工作";
	annotateDocPath.textContent = "";
}

function renderAgentWorks(items = allAgentWorkItems()) {
	treeEl.replaceChildren();
	workStatsEl.textContent = agentWorkItems.length
		? `${agentWorkItems.length} 个工作${localAgentRuns.length ? ` · ${localAgentRuns.length} 个运行中` : ""}`
		: localAgentRuns.length
			? `${localAgentRuns.length} 个运行中`
			: "还没有 Agent 工作";
	if (!items.length && !historicAgentFailures.length) {
		const tip = document.createElement("div");
		tip.className = "empty-tip agent-empty-tip";
		tip.textContent = searchInput.value.trim()
			? "没有匹配的 Agent 工作"
			: "还没有 Agent 工作。选中文字后点“问 Agent”，结果会在这里成为可编辑文档。";
		treeEl.appendChild(tip);
		return;
	}
	const fragment = document.createDocumentFragment();
	const visibleReceipts = historicAgentFailures.filter(
		(receipt) => !localAgentRuns.some((item) => item.id === receipt.runId),
	);
	for (const receipt of visibleReceipts) {
		fragment.appendChild(renderHistoricRunFailure(receipt));
	}
	if (!items.length && visibleReceipts.length) {
		const tip = document.createElement("div");
		tip.className = "empty-tip agent-empty-tip";
		tip.textContent = searchInput.value.trim()
			? "没有匹配的 Agent 工作"
			: "还没有 Agent 工作。选中文字后点“问 Agent”，结果会在这里成为可编辑文档。";
		fragment.appendChild(tip);
	}
	for (const item of items) {
		const row = document.createElement("div");
		row.className = `agent-work-item${item.id === currentAgentDocument?.id ? " active" : ""}`;
		row.classList.toggle(
			"agent-work-running",
			Boolean(item.local && !item.terminal),
		);
		const open = document.createElement("button");
		open.type = "button";
		open.className = "agent-work-open";
		const title = document.createElement("strong");
		title.className = "agent-work-title";
		title.textContent = item.title || "Agent 工作";
		const meta = document.createElement("span");
		meta.className = "agent-work-meta";
		const status = item.toolStatus || item.status || "已完成";
		meta.textContent = `${item.error ? `${status}：${item.error}` : status} · ${formatAgentTime(item.updatedAt)}`;
		const origin = document.createElement("span");
		origin.className = "agent-work-origin";
		origin.textContent = item.originQuote
			? `“${item.originQuote.slice(0, 56)}${item.originQuote.length > 56 ? "…" : ""}”`
			: item.originUri || item.prompt || "独立 Agent 工作";
		open.append(title, meta, origin);
		const previewText =
			item.preview ||
			item.streamText ||
			(item.error ? `错误：${item.error}` : "");
		if (previewText) {
			const preview = document.createElement("span");
			preview.className = "agent-work-preview";
			preview.textContent = previewText;
			open.appendChild(preview);
		}
		open.addEventListener("click", () => {
			if (item.local) return;
			void openAgentWork(item);
		});
		row.appendChild(open);
		if (item.local && agentBusy && !item.terminal) {
			const stop = document.createElement("button");
			stop.type = "button";
			stop.className = "agent-work-stop";
			stop.textContent = "停止";
			stop.title = "停止等待本地 Agent 进程";
			stop.addEventListener("click", () => void cancelAgentTurn());
			row.appendChild(stop);
		}
		fragment.appendChild(row);
	}
	treeEl.appendChild(fragment);
}

/// 持久化失败收据的只读展示：阶段 + 错误 + 时间，重启后仍然可见。
function renderHistoricRunFailure(receipt) {
	const rowEl = document.createElement("div");
	rowEl.className = "agent-work-item agent-run-receipt";
	rowEl.title =
		"上次运行失败的持久化收据（AppData/agent/runs），重启应用后仍可查";
	const open = document.createElement("div");
	open.className = "agent-work-open";
	const titleEl = document.createElement("strong");
	titleEl.className = "agent-work-title";
	titleEl.textContent = receipt.title || "未命名 Agent 运行";
	const meta = document.createElement("span");
	meta.className = "agent-work-meta";
	const parts = ["运行失败"];
	if (receipt.stage) parts.push(`阶段 ${receipt.stage}`);
	parts.push(formatAgentTime(receipt.endedAt || receipt.startedAt || 0));
	meta.textContent = parts.join(" · ");
	const detail = document.createElement("span");
	detail.className = "agent-work-preview";
	detail.textContent = receipt.error
		? `错误：${shortAgentError(receipt.error)}`
		: "未记录错误详情，完整收据见 AppData/agent/runs";
	open.append(titleEl, meta, detail);
	rowEl.appendChild(open);
	return rowEl;
}

async function loadAgentWorks() {
	if (!rootPath) {
		agentWorkItems = [];
		renderAgentWorks();
		return;
	}
	try {
		agentWorkItems = await invoke("list_agent_works");
		const query = searchInput.value.trim().toLocaleLowerCase();
		const items = query
			? allAgentWorkItems().filter((item) =>
					[item.title, item.prompt, item.originUri, item.originQuote]
						.filter(Boolean)
						.join("\n")
						.toLocaleLowerCase()
						.includes(query),
				)
			: allAgentWorkItems();
		renderAgentWorks(items);
	} catch (error) {
		console.error("读取 Agent 工作失败", error);
		agentWorkItems = [];
		renderAgentWorks();
	}
}

async function openAgentWork(item) {
	const token = ++loadToken;
	await saveCurrent();
	if (annotateDirty) await saveAnnotate();
	try {
		const data = await invoke("read_agent_work", { id: item.id });
		if (token !== loadToken) return;
		showAgentDocument(data);
	} catch (error) {
		console.error(error);
		markError("Agent 文档打开失败");
	}
}

function showAgentDocument(data) {
	clearRelated();
	clearWebSearch();
	currentAgentDocument = data;
	currentFile = null;
	currentLibraryDocument = null;
	// Agent Work 是中栏 Markdown Surface：退出 Work Surface（仍在「工作」标签）
	workSurfaceActive = false;
	editor.readOnly = false;
	editor.classList.remove("readonly");
	editorPaneLabel.textContent = "WRITE · AGENT";
	annotationButton.disabled = false;
	aggregateButton.disabled = true;
	editor.value = data.content || "";
	updatePreview();
	documentTitleEl.textContent = data.title || "Agent 工作";
	editor.scrollTop = 0;
	previewEl.scrollTop = 0;
	markSaved();
	loadAnnotationPanel();
	void loadWebSearchHistory();
	loadViewModeForCurrentDocument();
	if (workMode && legacyAgentView) renderAgentWorks();
	updateFormatToolbarState();
}

async function createFile(relativePath) {
	try {
		const path = await invoke("create_markdown", { relativePath });
		await refreshTree();
		await openFile(path, basename(path));
	} catch (error) {
		console.error(error);
		markError(String(error).includes("已存在") ? "文件已存在" : "新建失败");
	}
}

function setSyncState(text, kind) {
	syncStateEl.textContent = text;
	syncStateEl.classList.remove("ok", "error");
	if (kind) syncStateEl.classList.add(kind);
	syncStateEl.hidden = !text;
}

function scheduleAutoSync() {
	if (syncTimer) clearTimeout(syncTimer);
	syncTimer = setTimeout(doSync, 4000);
}

async function doSync() {
	if (!rootPath) return;
	setSyncState("同步中…");
	syncButton.disabled = true;
	try {
		const remote = localStorage.getItem("stillwrite.remote") || DEFAULT_REMOTE;
		const status = await invoke("sync_workspace", { remote });
		autoSync = true;
		setSyncState(status.message || "已同步", "ok");
		// 同步会改动磁盘文件，刷新目录树与索引
		await refreshTree();
	} catch (error) {
		console.error(error);
		const message = String(error).replace(
			/^Error invoking remote method '[^']+': Error: /,
			"",
		);
		setSyncState("同步失败", "error");
		syncStateEl.title = message;
	} finally {
		syncButton.disabled = false;
	}
}

function formatAnnotationTime(date = new Date()) {
	const pad = (value) => String(value).padStart(2, "0");
	return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

function annotationKindLabel(kind) {
	if (kind === "paragraph") return "段落";
	if (kind === "document") return "全文";
	return "字句";
}

function relativeDocumentPath(path) {
	if (!rootPath || !path) return path || "";
	const doc = path.replaceAll("\\", "/");
	const root = rootPath.replaceAll("\\", "/").replace(/\/$/, "");
	return doc.startsWith(`${root}/`) ? doc.slice(root.length + 1) : doc;
}

function autosizeAnnotationEditor(textarea) {
	textarea.style.height = "auto";
	textarea.style.height = `${Math.min(220, Math.max(68, textarea.scrollHeight))}px`;
}

function renderAnnotationPanel() {
	annotationList.replaceChildren();
	annotateCount.textContent = String(annotationItems.length);
	annotationEmpty.hidden =
		annotationItems.length > 0 || !annotateComposer.hidden;
	annotateHint.textContent = annotationItems.length
		? `${annotationItems.length} 条 · 点击原文可回到对应位置`
		: "选择字句或把光标放在段落中，再点“新批注”";

	annotationItems.forEach((item, index) => {
		const card = document.createElement("article");
		card.className = "annotation-card";
		card.dataset.annotationId = item.id;
		card.setAttribute("role", "listitem");
		card.classList.toggle("active", item.id === activeAnnotationId);

		const meta = document.createElement("div");
		meta.className = "annotation-card-meta";
		const ordinal = document.createElement("span");
		ordinal.className = "annotation-ordinal";
		ordinal.textContent = String(index + 1);
		const kind = document.createElement("span");
		kind.className = "annotation-kind";
		kind.textContent = annotationKindLabel(item.kind);
		const time = document.createElement("time");
		time.textContent = item.updatedAt || item.createdAt || "刚刚";
		meta.append(ordinal, kind, time);

		const quote = document.createElement("button");
		quote.type = "button";
		quote.className = "annotation-card-quote";
		quote.textContent = item.quote || "整篇文档";
		quote.title = "定位到原文";
		quote.addEventListener("click", () => focusAnnotation(item.id, true));

		const note = document.createElement("textarea");
		note.className = "annotation-card-note";
		note.value = item.note;
		note.rows = 2;
		note.spellcheck = true;
		note.setAttribute("aria-label", `第 ${index + 1} 条批注`);
		note.addEventListener("focus", () => focusAnnotation(item.id, false));
		note.addEventListener("input", () => {
			item.note = note.value;
			item.updatedAt = formatAnnotationTime();
			time.textContent = item.updatedAt;
			autosizeAnnotationEditor(note);
			markAnnotateDirty();
			scheduleAnnotateSave();
		});

		const actions = document.createElement("div");
		actions.className = "annotation-card-actions";
		const remove = document.createElement("button");
		remove.type = "button";
		remove.className = "annotation-delete";
		remove.textContent = "删除";
		remove.addEventListener("click", () => deleteAnnotation(item.id));
		actions.append(remove);

		card.append(meta, quote, note, actions);
		annotationList.appendChild(card);
		requestAnimationFrame(() => autosizeAnnotationEditor(note));
	});
}

function markdownQuoteToText(source) {
	return String(source || "")
		.split("\n")
		.map((line) =>
			line
				.replace(/^\s{0,3}#{1,6}\s+/, "")
				.replace(/^\s*>\s?/, "")
				.replace(/^\s*(?:[-+*]|\d+[.)])\s+/, ""),
		)
		.join(" ")
		.replace(/!\[([^\]]*)\]\([^)]*\)/g, "$1")
		.replace(/\[([^\]]+)\]\([^)]*\)/g, "$1")
		.replace(/[`*_~]/g, "")
		.trim();
}

function unwrapAnnotationHighlights() {
	previewEl
		.querySelectorAll(".annotation-marker")
		.forEach((node) => node.remove());
	[...previewEl.querySelectorAll("mark.annotation-highlight")]
		.reverse()
		.forEach((mark) => mark.replaceWith(...mark.childNodes));
	previewEl.normalize();
}

function visibleTextMap(root) {
	const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, {
		acceptNode(node) {
			return node.parentElement?.closest(".annotation-marker")
				? NodeFilter.FILTER_REJECT
				: NodeFilter.FILTER_ACCEPT;
		},
	});
	const positions = [];
	let normalized = "";
	let node;
	while ((node = walker.nextNode())) {
		for (let offset = 0; offset < node.data.length; offset += 1) {
			const char = node.data[offset];
			if (/\s/.test(char)) continue;
			normalized += char;
			positions.push({ node, offset });
		}
	}
	return { normalized, positions };
}

function addAnnotationMarker(target, item, ordinal) {
	const marker = document.createElement("button");
	marker.type = "button";
	marker.className = "annotation-marker";
	marker.dataset.annotationId = item.id;
	marker.textContent = String(ordinal);
	marker.title = `查看第 ${ordinal} 条批注`;
	marker.setAttribute("aria-label", marker.title);
	marker.classList.toggle("active", item.id === activeAnnotationId);
	marker.addEventListener("click", (event) => {
		event.preventDefault();
		event.stopPropagation();
		focusAnnotation(item.id, false);
	});
	const containingLink = target.closest?.("a");
	if (containingLink) containingLink.after(marker);
	else target.appendChild(marker);
}

function highlightAnnotation(item, ordinal) {
	const needle = markdownQuoteToText(item.quote).replace(/\s/g, "");
	if (!needle) {
		const target = previewEl.firstElementChild;
		if (target) addAnnotationMarker(target, item, ordinal);
		return Boolean(target);
	}
	const { normalized, positions } = visibleTextMap(previewEl);
	let occurrence = 0;
	if (item.start > 0 && item.quote) {
		let rawOffset = 0;
		while ((rawOffset = editor.value.indexOf(item.quote, rawOffset)) >= 0) {
			if (rawOffset >= item.start) break;
			occurrence += 1;
			rawOffset += Math.max(1, item.quote.length);
		}
	}
	let start = -1;
	let normalizedOffset = 0;
	for (let index = 0; index <= occurrence; index += 1) {
		start = normalized.indexOf(needle, normalizedOffset);
		if (start < 0) break;
		normalizedOffset = start + Math.max(1, needle.length);
	}
	if (start < 0) return false;
	const matched = positions.slice(start, start + needle.length);
	const groups = [];
	matched.forEach(({ node, offset }) => {
		let group = groups[groups.length - 1];
		if (!group || group.node !== node) {
			group = { node, start: offset, end: offset + 1 };
			groups.push(group);
		} else {
			group.end = offset + 1;
		}
	});
	const wrappers = [];
	groups.reverse().forEach((group) => {
		if (!group.node.isConnected) return;
		const range = document.createRange();
		range.setStart(group.node, group.start);
		range.setEnd(group.node, group.end);
		const mark = document.createElement("mark");
		mark.className = "annotation-highlight";
		mark.dataset.annotationId = item.id;
		mark.classList.toggle("active", item.id === activeAnnotationId);
		mark.appendChild(range.extractContents());
		range.insertNode(mark);
		wrappers.unshift(mark);
	});
	if (!wrappers.length) return false;
	addAnnotationMarker(wrappers[wrappers.length - 1], item, ordinal);
	return true;
}

function renderAnnotationAnchors() {
	unwrapAnnotationHighlights();
	if (annotateLoadedDoc !== documentRefKey()) return;
	annotationItems.forEach((item, index) => {
		const resolved = AnnotationCodec.resolveRange(editor.value, item);
		if (resolved.start >= 0) {
			item.start = resolved.start;
			item.end = resolved.end;
		}
		highlightAnnotation(item, index + 1);
	});
}

function focusAnnotation(id, revealSource) {
	activeAnnotationId = id;
	if (!annotateVisible) setAnnotateVisible(true);
	document.querySelectorAll("[data-annotation-id]").forEach((element) => {
		element.classList.toggle("active", element.dataset.annotationId === id);
	});
	const card = annotationList.querySelector(
		`[data-annotation-id="${CSS.escape(id)}"]`,
	);
	card?.scrollIntoView({ block: "nearest", behavior: "smooth" });
	if (!revealSource) return;
	const marker = previewEl.querySelector(
		`.annotation-marker[data-annotation-id="${CSS.escape(id)}"]`,
	);
	if (marker) {
		marker.scrollIntoView({ block: "center", behavior: "smooth" });
		return;
	}
	const item = annotationItems.find((entry) => entry.id === id);
	if (!item) return;
	const range = AnnotationCodec.resolveRange(editor.value, item);
	if (range.start >= 0) {
		editor.focus();
		editor.setSelectionRange(range.start, range.end);
	}
}

function captureEditorAnnotation() {
	const range = AnnotationCodec.selectionOrParagraph(
		editor.value,
		editor.selectionStart,
		editor.selectionEnd,
	);
	return range.quote.trim() ? range : null;
}

function captureEditorSelection() {
	const start = editor.selectionStart;
	const end = editor.selectionEnd;
	if (!(end > start)) return null;
	const range = AnnotationCodec.selectionOrParagraph(editor.value, start, end);
	return range.quote.trim() ? range : null;
}

function renderAgentAskRefs() {
	const hits = [...citationBasket.values()];
	agentAskRefList.replaceChildren();
	for (const hit of hits) {
		const item = document.createElement("li");
		item.textContent = hit.title || hit.relative_path || hit.uri;
		agentAskRefList.appendChild(item);
	}
	agentAskRefs.hidden = hits.length === 0;
}

async function beginAgentQuestion(range = null, { allowEmpty = false } = {}) {
	const target = currentDocumentRef();
	if (!target && !allowEmpty) {
		saveStateEl.textContent = "请先打开文档";
		return;
	}
	const captured = range || (target ? captureEditorAnnotation() : null);
	if (target && !captured?.quote?.trim() && !allowEmpty) {
		saveStateEl.textContent = "请先选择字句，或把光标放进一个段落";
		return;
	}
	pendingAgentRequest = {
		target,
		originUri: target ? currentDocumentUri() : null,
		originQuote: captured?.quote?.trim() || null,
	};
	agentAskTitle.textContent = pendingAgentRequest.originQuote
		? "问 Agent"
		: "新 Agent 工作";
	agentAskContext.hidden = !pendingAgentRequest.originQuote;
	agentAskQuote.textContent = pendingAgentRequest.originQuote || "";
	renderAgentAskRefs();
	agentAskPrompt.value = "";
	agentAskSend.disabled = false;
	agentAskDialog.showModal();
	requestAnimationFrame(() => agentAskPrompt.focus());
}

function addCompletedAgentWork(document) {
	const summary = {
		id: document.id,
		uri: document.uri,
		title: document.title,
		prompt: document.prompt,
		originUri: document.originUri,
		originQuote: document.originQuote,
		createdAt: document.createdAt,
		updatedAt: document.updatedAt,
		status: document.status || "已完成",
		piSessionRef: document.piSessionRef || null,
	};
	agentWorkItems = [
		summary,
		...agentWorkItems.filter((item) => item.id !== summary.id),
	];
}

async function submitAgentQuestion(event) {
	event.preventDefault();
	if (agentBusy) return;
	const prompt = agentAskPrompt.value.trim();
	if (!prompt) {
		agentAskPrompt.focus();
		return;
	}
	const request = pendingAgentRequest || { originUri: null, originQuote: null };
	agentAskDialog.close();
	if (!rootPath) {
		await chooseWorkspace();
		if (!rootPath) return;
	}
	await saveCurrent();
	const run = makeLocalAgentRun(request, prompt);
	localAgentRuns.unshift(run);
	activeAgentRunId = run.id;
	agentBusy = true;
	agentCancelRequested = false;
	if (workMode && legacyAgentView) renderAgentWorks();
	saveStateEl.textContent = "Agent 运行中…";
	saveStateEl.classList.remove("error");
	const completion = waitForAgentRun(run.id);
	try {
		await installAgentEventListener();
		if (agentCancelRequested) throw new Error("Agent 已停止");
		await probeAgent();
		if (agentCancelRequested) throw new Error("Agent 已停止");
		const citationContext = await loadCitationContext();
		if (agentCancelRequested) throw new Error("Agent 已停止");
		const response = await invoke("agent_start", {
			input: {
				runId: run.id,
				title: run.title,
				prompt: buildAgentPrompt(request, prompt, citationContext),
			},
		});
		if (!response?.accepted) {
			const error = new Error("Pi 没有接受 Agent 请求");
			agentRunWaiters.delete(run.id);
			throw error;
		}
		run.piSessionRef = response.piSessionRef || null;
		// Cancellation can race with the setup commands inside agent_start.
		// If the backend accepted after the first abort saw no active run, make
		// one cleanup attempt now so that no orphaned Pi turn can continue.
		if (agentCancelRequested) {
			try {
				await invoke("agent_abort");
			} catch (error) {
				console.warn("清理已取消的 Agent 失败", error);
			}
			throw new Error("Agent 已停止");
		}
		const terminal = await completion;
		if (terminal.type !== "agent_settled") {
			throw new Error(
				terminal.message ||
					(terminal.type === "agent_stopped"
						? "Agent 已停止"
						: "Agent 运行失败"),
			);
		}
		if (agentCancelRequested) throw new Error("Agent 已停止");
		const title = run.title;
		const document = await invoke("create_agent_work", {
			input: {
				title,
				content: agentDocumentContent(terminal.text, title),
				prompt,
				originUri: request.originUri,
				originQuote: request.originQuote,
				piSessionRef: terminal.piSessionRef || run.piSessionRef || null,
				runId: run.id,
			},
		});
		addCompletedAgentWork(document);
		localAgentRuns = localAgentRuns.filter((item) => item.id !== run.id);
		if (workMode && legacyAgentView) renderAgentWorks();
		if (workMode) void loadWorks();
		saveStateEl.textContent = "Agent 工作已完成";
	} catch (error) {
		console.error("Agent 工作失败", error);
		agentRunWaiters.delete(run.id);
		const local = localAgentRuns.find((item) => item.id === run.id);
		if (local) {
			local.status = agentCancelRequested ? "已停止" : "失败";
			local.terminal = true;
			local.error = String(error);
			local.updatedAt = Math.floor(Date.now() / 1000);
		}
		if (workMode && legacyAgentView) renderAgentWorks();
		if (workMode) void loadWorks();
		markError(
			agentCancelRequested
				? "Agent 已停止"
				: `Agent 失败：${shortAgentError(error)}`,
		);
		void loadHistoricAgentFailures();
	} finally {
		agentBusy = false;
		activeAgentRunId = null;
		agentCancelRequested = false;
		pendingAgentRequest = null;
		if (workMode && legacyAgentView) renderAgentWorks();
	}
}

async function beginAnnotation(range = null) {
	const target = currentDocumentRef();
	if (
		!target ||
		(target.kind === "workspace" && !isAnnotatablePath(currentFile))
	) {
		annotateFoot.textContent = currentFile
			? "批注文件本身不能再批注"
			: "请先打开一篇文档";
		return;
	}
	const captured = range || captureEditorAnnotation();
	if (!captured?.quote?.trim()) {
		annotateFoot.textContent = "请先选择字句，或把光标放进一个段落";
		return;
	}
	if (annotateLoadedDoc !== documentRefKey(target)) await loadAnnotationPanel();
	if (annotateLoadedDoc !== documentRefKey(target)) {
		annotateFoot.textContent = "批注尚未加载完成，请稍后重试";
		return;
	}
	pendingAnnotation = captured;
	setAnnotateVisible(true);
	annotateComposer.hidden = false;
	annotationEmpty.hidden = true;
	annotationDraftQuote.textContent = captured.quote;
	annotationDraft.value = "";
	requestAnimationFrame(() => {
		autosizeAnnotationEditor(annotationDraft);
		annotationDraft.focus();
	});
}

function cancelAnnotation() {
	pendingAnnotation = null;
	annotateComposer.hidden = true;
	annotationDraft.value = "";
	annotationEmpty.hidden = annotationItems.length > 0;
}

function addPendingAnnotation() {
	const note = annotationDraft.value.trim();
	if (!pendingAnnotation || !note) {
		annotationDraft.focus();
		return;
	}
	const now = formatAnnotationTime();
	const item = AnnotationCodec.normalizeItem({
		...pendingAnnotation,
		id: AnnotationCodec.newId(),
		note,
		createdAt: now,
		updatedAt: now,
	});
	annotationItems.push(item);
	activeAnnotationId = item.id;
	cancelAnnotation();
	renderAnnotationPanel();
	renderAnnotationAnchors();
	markAnnotateDirty();
	saveAnnotate();
	requestAnimationFrame(() => focusAnnotation(item.id, false));
}

function deleteAnnotation(id) {
	const item = annotationItems.find((entry) => entry.id === id);
	if (!item || !window.confirm("删除这条批注？此操作会立即保存。")) return;
	annotationItems = annotationItems.filter((entry) => entry.id !== id);
	if (activeAnnotationId === id) activeAnnotationId = null;
	renderAnnotationPanel();
	renderAnnotationAnchors();
	markAnnotateDirty();
	saveAnnotate();
}

// 读取当前文档的批注；旧版整篇自由文本会自动转换为“全文批注”。
async function loadAnnotationPanel() {
	const loadTokenForAnnotation = ++annotationLoadToken;
	const target = currentDocumentRef();
	const requestedKey = documentRefKey(target);
	const libraryDocument =
		target?.kind === "library" ? currentLibraryDocument : null;
	const agentDocument = target?.kind === "agent" ? currentAgentDocument : null;
	if (!target) {
		annotateDocName.textContent = "未打开文档";
		annotateDocPath.textContent = "";
		annotationItems = [];
		annotateLoadedDoc = null;
		activeAnnotationId = null;
		cancelAnnotation();
		renderAnnotationPanel();
		annotateSaveState.textContent = "";
		annotateSaveState.classList.remove("dirty", "error");
		return;
	}
	try {
		const data = await invoke("read_annotation", { target });
		if (
			loadTokenForAnnotation !== annotationLoadToken ||
			requestedKey !== documentRefKey()
		)
			return;
		annotationItems = AnnotationCodec.parse(
			data.body || "",
			data.updated_at || "",
		);
		annotateLoadedDoc = requestedKey;
		activeAnnotationId = null;
		annotateDocName.textContent =
			target.kind === "workspace"
				? basename(target.path).replace(/\.(md|markdown)$/i, "")
				: target.kind === "library"
					? data.title || libraryDocument?.title || target.relativePath
					: data.title || agentDocument?.title || "Agent 工作";
		annotateDocPath.textContent =
			target.kind === "workspace"
				? relativeDocumentPath(target.path)
				: target.kind === "library"
					? data.doc_path || libraryDocument?.uri || target.relativePath
					: data.doc_path || agentDocument?.uri || target.id;
		annotateDocPath.title =
			target.kind === "workspace"
				? target.path
				: target.kind === "library"
					? data.doc_path || libraryDocument?.uri || target.relativePath
					: data.doc_path || agentDocument?.uri || target.id;
		annotateDirty = false;
		cancelAnnotation();
		renderAnnotationPanel();
		renderAnnotationAnchors();
		annotateSaveState.textContent = annotationItems.length ? "已保存" : "";
		annotateSaveState.classList.remove("dirty", "error");
		annotateFoot.textContent =
			target.kind === "workspace"
				? "批注保存在「批注/」，随文档同步。"
				: target.kind === "library"
					? "资料正文只读；批注保存在 StillWrite 应用数据中。"
					: "Agent 工作可编辑；批注保存在 StillWrite 应用数据中。";
	} catch (error) {
		if (
			loadTokenForAnnotation !== annotationLoadToken ||
			requestedKey !== documentRefKey()
		)
			return;
		console.error(error);
		annotationItems = [];
		// 读取失败时不能把文档标记为已加载，否则“新批注”会进入内存，
		// 随后保存必然失败，还会留下看似属于“未打开文档”的批注。
		annotateLoadedDoc = null;
		activeAnnotationId = null;
		cancelAnnotation();
		annotateDocName.textContent =
			target.kind === "workspace"
				? basename(target.path).replace(/\.(md|markdown)$/i, "")
				: target.kind === "library"
					? libraryDocument?.title || target.relativePath
					: agentDocument?.title || "Agent 工作";
		annotateDocPath.textContent =
			target.kind === "workspace"
				? relativeDocumentPath(target.path)
				: target.kind === "library"
					? libraryDocument?.uri || target.relativePath
					: agentDocument?.uri || target.id;
		annotateDocPath.title =
			target.kind === "workspace"
				? target.path
				: target.kind === "library"
					? libraryDocument?.uri || target.relativePath
					: agentDocument?.uri || target.id;
		renderAnnotationPanel();
		renderAnnotationAnchors();
		annotateSaveState.textContent = "读取批注失败";
		annotateSaveState.classList.remove("dirty");
		annotateSaveState.classList.add("error");
		annotateSaveState.title = String(error);
		annotateFoot.textContent = String(error).includes("不能再写批注")
			? "批注文件本身不能再批注"
			: target.kind === "library"
				? "资料批注读取失败，请刷新资料库后重试"
				: target.kind === "agent"
					? "Agent 批注读取失败"
					: "读取批注失败";
	}
}

function markAnnotateDirty() {
	annotateDirty = true;
	annotateSaveState.textContent = "编辑中…";
	annotateSaveState.classList.add("dirty");
	annotateSaveState.classList.remove("error");
}

function scheduleAnnotateSave() {
	if (!currentDocumentRef()) return;
	if (annotateTimer) clearTimeout(annotateTimer);
	annotateTimer = setTimeout(saveAnnotate, 650);
}

async function saveAnnotate() {
	const target = currentDocumentRef();
	if (!target || annotateLoadedDoc !== documentRefKey(target)) return;
	if (annotateTimer) {
		clearTimeout(annotateTimer);
		annotateTimer = null;
	}
	const body = AnnotationCodec.serialize(annotationItems);
	try {
		await invoke("save_annotation", { target, body });
		annotateSaveState.textContent = annotationItems.length ? "已保存" : "";
		annotateSaveState.classList.remove("dirty", "error");
		annotateDirty = false;
		if (target.kind === "workspace" && autoSync) scheduleAutoSync();
	} catch (error) {
		console.error(error);
		annotateSaveState.textContent = "保存失败";
		annotateSaveState.classList.add("error");
	}
}

function hideSelectionAnnotate() {
	selectionActions.hidden = true;
	pendingPreviewSelection = null;
}

function positionSelectionActions(rect, { below = false } = {}) {
	selectionActions.hidden = false;
	const bounds = selectionActions.getBoundingClientRect();
	const width = bounds.width || 260;
	const height = bounds.height || 34;
	const maxLeft = Math.max(8, window.innerWidth - width - 8);
	const maxTop = Math.max(8, window.innerHeight - height - 8);
	const left = Math.min(
		maxLeft,
		Math.max(8, rect.left + rect.width / 2 - width / 2),
	);
	const desiredTop = below ? rect.top + 12 : rect.top - height - 8;
	const top = Math.min(maxTop, Math.max(8, desiredTop));
	selectionActions.style.left = `${left}px`;
	selectionActions.style.top = `${top}px`;
}

function sourceRangeForPreviewSelection(quote) {
	const exact = editor.value.indexOf(quote);
	if (exact >= 0)
		return {
			kind: "selection",
			start: exact,
			end: exact + quote.length,
			quote,
		};
	return { kind: "selection", start: -1, end: -1, quote };
}

function showPreviewSelectionAction() {
	const selection = window.getSelection();
	if (!selection || selection.isCollapsed || !selection.rangeCount) {
		hideSelectionAnnotate();
		return;
	}
	const range = selection.getRangeAt(0);
	const container =
		range.commonAncestorContainer.nodeType === Node.ELEMENT_NODE
			? range.commonAncestorContainer
			: range.commonAncestorContainer.parentElement;
	if (!container || !previewEl.contains(container)) {
		hideSelectionAnnotate();
		return;
	}
	const quote = selection.toString().trim();
	if (!quote || quote.length > 4000) {
		hideSelectionAnnotate();
		return;
	}
	pendingPreviewSelection = sourceRangeForPreviewSelection(quote);
	selectionRelatedButton.hidden = !isWorkspaceDocumentForRelated();
	const rect = range.getBoundingClientRect();
	positionSelectionActions(rect);
}

function showEditorSelectionAction() {
	const range = captureEditorSelection();
	if (!range || range.quote.length > 4000) {
		hideSelectionAnnotate();
		return;
	}
	pendingPreviewSelection = range;
	selectionRelatedButton.hidden = !isWorkspaceDocumentForRelated();
	const rect = editor.getBoundingClientRect();
	positionSelectionActions(rect, { below: true });
}

function describeBraveApiKeyStatus(source) {
	if (source === "settings") return "已配置（输入新 Key 可替换）";
	if (source === "env") return "当前使用环境变量；保存后将优先使用设置中的 Key";
	return "尚未配置";
}

async function refreshBraveApiKeyStatus() {
	try {
		const source = await invoke("brave_api_key_status");
		braveApiKeyStatus.textContent = describeBraveApiKeyStatus(source);
		return source;
	} catch (error) {
		console.error("读取 Brave API Key 配置状态失败", error);
		braveApiKeyStatus.textContent = `读取配置失败：${String(error)}`;
		return null;
	}
}

async function openSettings() {
	braveApiKeyInput.value = "";
	braveApiKeyStatus.textContent = "读取配置中…";
	if (!settingsDialog.open) settingsDialog.showModal();
	await refreshBraveApiKeyStatus();
	requestAnimationFrame(() => braveApiKeyInput.focus());
}

async function saveSettings() {
	const key = braveApiKeyInput.value.trim();
	if (!key) {
		braveApiKeyStatus.textContent =
			"请输入 API Key；不修改请取消，移除请点击清除";
		braveApiKeyInput.focus();
		return;
	}
	saveSettingsButton.disabled = true;
	clearBraveApiKeyButton.disabled = true;
	braveApiKeyStatus.textContent = "保存中…";
	try {
		await invoke("save_brave_api_key", { key });
		settingsDialog.close();
	} catch (error) {
		console.error("保存 Brave API Key 失败", error);
		braveApiKeyStatus.textContent = `保存失败：${String(error)}`;
	} finally {
		saveSettingsButton.disabled = false;
		clearBraveApiKeyButton.disabled = false;
	}
}

async function clearBraveApiKey() {
	clearBraveApiKeyButton.disabled = true;
	saveSettingsButton.disabled = true;
	braveApiKeyStatus.textContent = "清除中…";
	try {
		await invoke("clear_brave_api_key");
		braveApiKeyInput.value = "";
		await refreshBraveApiKeyStatus();
	} catch (error) {
		console.error("清除 Brave API Key 失败", error);
		braveApiKeyStatus.textContent = `清除失败：${String(error)}`;
	} finally {
		clearBraveApiKeyButton.disabled = false;
		saveSettingsButton.disabled = false;
	}
}

async function searchWebFromSelection(range) {
	const query = range?.quote?.trim();
	if (!query) return;
	const requestToken = ++webSearchRequestToken;
	const historyToken = ++webSearchHistoryToken;
	const documentUri = currentDocumentUri();
	setAnnotateVisible(true);
	renderWebSearchMessage("正在通过 Brave 搜索互联网…");
	setSupportView("web-search");
	try {
		const history = await invoke("search_web", {
			query,
			count: 10,
			documentUri,
			selectedText: range.quote,
		});
		if (
			requestToken !== webSearchRequestToken ||
			historyToken !== webSearchHistoryToken
		)
			return;
		webSearchHistory = [
			history,
			...webSearchHistory.filter((item) => item.id !== history.id),
		];
		expandedWebSearchIds.add(history.id);
		webSearchMessage = "";
		renderWebSearchToolbar();
		await refreshWebSearchLinks();
		if (
			requestToken === webSearchRequestToken &&
			historyToken === webSearchHistoryToken
		)
			renderWebSearchState();
	} catch (error) {
		if (
			requestToken !== webSearchRequestToken ||
			historyToken !== webSearchHistoryToken
		)
			return;
		console.error("Brave 互联网搜索失败", error);
		renderWebSearchMessage(
			String(error).includes("未配置")
				? "未配置 Brave API Key，请点击左下角 ⚙ 设置"
				: `Brave 搜索失败：${String(error)}`,
		);
	}
}

// 源文档是否允许批注（批注文件与汇总文件自身不能再批注）
function isAnnotatablePath(path) {
	if (!path) return false;
	const normalized = path.replaceAll("\\", "/");
	return !(
		normalized.endsWith("/批注汇总.md") || normalized.includes("/批注/")
	);
}

function samePath(a, b) {
	return a.replaceAll("\\", "/") === b.replaceAll("\\", "/");
}

async function doAggregateAnnotations() {
	if (!rootPath) return;
	// Workspace 汇总只聚合 Workspace 批注 sidecar：无论当前 surface 是
	// Work / Library / Agent 都可以执行。先落盘当前 Workspace 文档的待保存
	// 批注（刚打完字就点汇总时，650ms 防抖可能还没触发）。
	if (currentFile && isAnnotatablePath(currentFile) && annotateDirty)
		await saveAnnotate();
	aggregateButton.disabled = true;
	annotateFoot.textContent = "汇总中…";
	try {
		const result = await invoke("aggregate_annotations");
		annotateFoot.textContent = `已汇总 ${result.count} 篇批注 → 批注汇总.md`;
		await refreshTree();
		// 若当前打开的文档就是汇总文件，从磁盘重读：
		// 否则编辑器里的陈旧缓冲区会在下次保存时把新汇总内容覆盖回去。
		if (currentFile && result.path && samePath(currentFile, result.path)) {
			const text = await invoke("read_markdown", { path: currentFile });
			showDocument(currentFile, basename(currentFile), text, null);
		}
	} catch (error) {
		console.error(error);
		annotateFoot.textContent = "汇总失败";
	} finally {
		aggregateButton.disabled = false;
	}
}

function updateRightPanelHandle() {
	annotateHandle.classList.toggle("hidden", !annotateVisible);
	annotateHandle.title = "拖动调整支持栏";
}

function setAnnotateVisible(visible) {
	annotateVisible = visible;
	localStorage.setItem("stillwrite.annotateVisible", String(visible));
	annotationButton.classList.toggle("active", visible);
	annotatePanel.hidden = !visible;
	annotatePanel.classList.toggle("hidden", !visible);
	document.documentElement.style.setProperty(
		"--annotate-width",
		`${annotateWidth}px`,
	);
	updateRightPanelHandle();
	if (visible && annotateLoadedDoc !== documentRefKey()) loadAnnotationPanel();
}

// ---------------------------------------------------------------------------
// 文档格式工具栏：第二行格式条 + 标题菜单 + 快捷键 + 本地图片预览水合。
// Markdown 变换逻辑全部在 markdown-formatting.js；这里只做 orchestration。
// ---------------------------------------------------------------------------

let headingMenuOpen = false;
const localImagePreviewCache = new Map();
const MAX_LOCAL_IMAGE_BYTES = 15 * 1024 * 1024;

/// 判断当前文档是否允许正文格式变换（Workspace 或 Agent Work 的可编辑正文）。
function formatEditableContext() {
	if (currentLibraryDocument) return false;
	if (!editor) return false;
	if (editor.readOnly) return false;
	if (!currentFile && !currentAgentDocument) return false;
	if (viewMode === "reader") return false; // 仅阅读模式：正文变换禁用
	return true;
}

/// Workspace 可上传本地图片；Agent Work 的 AppData 正文 v0.1 不建 asset store。
function canUploadWorkspaceImage() {
	return Boolean(currentFile && !currentLibraryDocument && !currentAgentDocument);
}

function updateFormatToolbarState() {
	const editable = formatEditableContext();
	const upload = canUploadWorkspaceImage();
	// Library / Work 浏览模式下即使上次文档引用残留，格式栏也整体禁用
	const browsingInactive = libraryMode || workMode;
	const hidden =
		!currentFile && !currentLibraryDocument && !currentAgentDocument;
	formatToolbar.hidden = hidden;
	shell.dataset.formatToolbar = hidden ? "hidden" : "shown";
	if (hidden) {
		closeHeadingMenu();
		return;
	}
	const writeCommands = [
		"bold",
		"italic",
		"strikethrough",
		"heading",
		"code",
		"quote",
		"list",
		"ordered-list",
		"check-list",
		"link",
		"image",
		"upload-image",
		"table",
		"hr",
	];
	for (const command of writeCommands) {
		const btn = formatButtons.get(command);
		if (!btn) continue;
		if (command === "upload-image") {
			btn.disabled = !upload || browsingInactive;
		} else {
			btn.disabled = !editable || browsingInactive;
		}
	}
	// comment 始终可用（复用现有 Annotation 流程）
	const commentBtn = formatButtons.get("comment");
	if (commentBtn) commentBtn.disabled = false;
	// File menu 的启用矩阵与文档/工作区状态同步刷新
	updateFileMenuState();
}

function textareaSelection() {
	return { selectionStart: editor.selectionStart, selectionEnd: editor.selectionEnd };
}

function restoreEditorSelection(selectionStart, selectionEnd) {
	editor.focus();
	editor.setSelectionRange(selectionStart, selectionEnd);
}

function onEditorContentChanged() {
	schedulePreview();
	markDirty();
	scheduleSave();
	if (isWorkspaceDocumentForRelated()) scheduleRelatedRefresh();
	setTimeout(showEditorSelectionAction);
}

function applyEditorResult(result) {
	editor.value = result.value;
	restoreEditorSelection(result.selectionStart, result.selectionEnd);
	onEditorContentChanged();
}

function applyFormatCommand(command, options = {}) {
	if (command === "comment") {
		void beginAnnotation();
		return;
	}
	if (command === "upload-image") {
		void uploadWorkspaceImage();
		return;
	}
	if (!formatEditableContext() && command !== "heading") return;
	const result = window.StillwriteMarkdownFormatting.applyCommand({
		value: editor.value,
		selectionStart: editor.selectionStart,
		selectionEnd: editor.selectionEnd,
		command,
		options,
	});
	if (!result || result.value === editor.value && command !== "heading") return;
	applyEditorResult(result);
}

async function uploadWorkspaceImage() {
	if (!canUploadWorkspaceImage()) return;
	try {
		const imported = await invoke("import_workspace_image", {
			documentPath: currentFile,
		});
		if (!imported) return; // 用户取消选择
		const result = window.StillwriteMarkdownFormatting.applyCommand({
			value: editor.value,
			selectionStart: editor.selectionStart,
			selectionEnd: editor.selectionEnd,
			command: "image-upload",
			options: { alt: imported.alt, markdownPath: imported.markdown_path },
		});
		if (!result) return;
		applyEditorResult(result);
	} catch (error) {
		console.warn("上传图片失败", error);
		markError(String(error));
	}
}

// ---------- 标题菜单 ----------

function openHeadingMenu() {
	headingMenuOpen = true;
	headingMenu.hidden = false;
	headingButton.classList.add("active");
	updateHeadingMenuHighlight();
}

function closeHeadingMenu() {
	headingMenuOpen = false;
	headingMenu.hidden = true;
	headingButton.classList.remove("active");
}

function toggleHeadingMenu() {
	if (headingMenuOpen) closeHeadingMenu();
	else openHeadingMenu();
}

function currentLineHeadingLevel() {
	const start = editor.selectionStart;
	const lineStart = editor.value.lastIndexOf("\n", Math.max(0, start - 1)) + 1;
	const line = editor.value.slice(lineStart, editor.value.indexOf("\n", start) === -1 ? undefined : editor.value.indexOf("\n", start));
	const match = line.match(/^\s{0,3}(#{1,6})\s+/);
	return match ? match[1].length : 0;
}

function updateHeadingMenuHighlight() {
	const level = currentLineHeadingLevel();
	headingMenu.querySelectorAll("[data-heading-level]").forEach((btn) => {
		btn.classList.toggle(
			"active",
			Number(btn.dataset.headingLevel) === level,
		);
	});
}

function handleHeadingLevel(level) {
	closeHeadingMenu();
	if (!formatEditableContext()) return;
	applyFormatCommand("heading", { level });
}

formatToolbar.addEventListener("click", (event) => {
	const button = event.target.closest(".format-btn");
	if (!button) return;
	const command = button.dataset.formatCommand;
	if (command === "heading") {
		toggleHeadingMenu();
		return;
	}
	closeHeadingMenu();
	applyFormatCommand(command);
});

headingMenu.addEventListener("click", (event) => {
	const button = event.target.closest("[data-heading-level]");
	if (!button) return;
	handleHeadingLevel(Number(button.dataset.headingLevel));
});

document.addEventListener("pointerdown", (event) => {
	if (headingMenuOpen && !event.target.closest("#headingMenu, [data-format-command=\"heading\"]")) {
		closeHeadingMenu();
	}
});

// ---------------------------------------------------------------------------
// 页边距调节：右下角悬浮 − / ＋ 控件（写 / 双栏 / 读三种模式均可用）。
// 写模式调编辑区（#editor），读/双栏模式调阅读区（.markdown-body）。
// 纯展示偏好（AGENTS.md 2.1 允许 localStorage），不进入任何 durable state。
// 默认不设置变量时维持原有 clamp(...) 响应式行为；设置后固定像素。
// 注意：阅读区内容列默认 max-width: 760px 居中，页边距超过 (窗宽 − 760) / 2
// 才会推动文字；因此用户设过固定值后，单栏读模式放开版心（CSS: body.margin-
// fixed），让任何数值立即可见——760px 版心只属于 auto，不纳入按键管理。
// ---------------------------------------------------------------------------

const MARGIN_STORAGE_KEY = "stillwrite.markdownPaddingX";
const MARGIN_MIN = 16;
const MARGIN_MAX = 480;
const MARGIN_STEP = 8;
// 从 auto 首次点击的起步值：取写/双栏 clamp 在常见窗宽下的典型生效量，
// 保证首击方向与按钮语义一致（− 变窄、＋ 变宽），而不是从极值跳变。
const MARGIN_AUTO_SEED = 72;
let markdownPaddingX = parseInt(
	localStorage.getItem(MARGIN_STORAGE_KEY),
	10,
);
if (Number.isNaN(markdownPaddingX)) markdownPaddingX = null;

const marginControl = document.querySelector("#marginControl");
const marginNarrowBtn = document.querySelector("#marginNarrow");
const marginWideBtn = document.querySelector("#marginWide");
const marginValueEl = document.querySelector("#marginValue");

function applyMarkdownPadding() {
	const rootStyle = document.documentElement.style;
	// margin-fixed：用户设过固定值。读单模式据此放开 760px 居中版心，
	// 让任何边距数值都立即可见；auto（null）保持居中阅读版心。
	if (markdownPaddingX === null) {
		rootStyle.removeProperty("--markdown-padding-x");
		rootStyle.removeProperty("--editor-padding-x");
		document.body.classList.remove("margin-fixed");
		marginValueEl.textContent = "auto";
	} else {
		rootStyle.setProperty("--markdown-padding-x", `${markdownPaddingX}px`);
		rootStyle.setProperty("--editor-padding-x", `${markdownPaddingX}px`);
		document.body.classList.add("margin-fixed");
		marginValueEl.textContent = String(markdownPaddingX);
	}
}

function setMarkdownPaddingX(value) {
	markdownPaddingX = Math.max(MARGIN_MIN, Math.min(MARGIN_MAX, value));
	localStorage.setItem(MARGIN_STORAGE_KEY, String(markdownPaddingX));
	applyMarkdownPadding();
}

marginNarrowBtn.addEventListener("click", () => {
	setMarkdownPaddingX(
		Math.max(MARGIN_MIN, (markdownPaddingX ?? MARGIN_AUTO_SEED) - MARGIN_STEP),
	);
});
marginWideBtn.addEventListener("click", () => {
	setMarkdownPaddingX(
		Math.min(MARGIN_MAX, (markdownPaddingX ?? MARGIN_AUTO_SEED) + MARGIN_STEP),
	);
});
applyMarkdownPadding();


function clearWebSearch({ resetView = true, clearHistory = false } = {}) {
	webSearchRequestToken += 1;
	webSearchHistoryToken += 1;
	webSearchMessage = "";
	if (clearHistory) {
		webSearchHistory = [];
		expandedWebSearchIds.clear();
	}
	linkedWebSearchResultIds.clear();
	webSearchCount.textContent = String(webSearchHistory.length);
	webSearchStream.replaceChildren();
	if (resetView && supportView === "web-search") setSupportView("annotation");
}

async function refreshWebSearchLinks() {
	const token = webSearchHistoryToken;
	linkedWebSearchResultIds.clear();
	const sourceUri = currentDocumentUri();
	if (!sourceUri) return;
	try {
		const ids = await invoke("list_search_result_links", { sourceUri });
		if (token !== webSearchHistoryToken) return;
		for (const id of ids || []) linkedWebSearchResultIds.add(Number(id));
	} catch (error) {
		console.warn("读取网页搜索关联失败", error);
	}
}

async function loadWebSearchHistory() {
	const token = ++webSearchHistoryToken;
	if (!rootPath) {
		clearWebSearch({ resetView: false, clearHistory: true });
		return;
	}
	try {
		const histories = await invoke("list_web_search_history", { limit: 50 });
		if (token !== webSearchHistoryToken) return;
		webSearchHistory = Array.isArray(histories) ? histories : [];
		const validIds = new Set(webSearchHistory.map((history) => history.id));
		for (const id of [...expandedWebSearchIds]) {
			if (!validIds.has(id)) expandedWebSearchIds.delete(id);
		}
		await refreshWebSearchLinks();
		if (token !== webSearchHistoryToken) return;
		webSearchMessage = "";
		renderWebSearchToolbar();
		if (supportView === "web-search") renderWebSearchState();
	} catch (error) {
		if (token !== webSearchHistoryToken) return;
		console.warn("读取网页搜索历史失败", error);
		webSearchMessage = "读取网页搜索历史失败，请稍后重试";
		renderWebSearchToolbar();
		if (supportView === "web-search") renderWebSearchState();
	}
}

function clearSearch() {
	searchInput.value = "";
	if (libraryMode) {
		renderLibraryHome();
	} else if (workMode) {
		if (legacyAgentView) renderAgentWorks();
		else renderWorkList();
	} else if (lastTreeNodes.length) {
		renderTree(lastTreeNodes);
	} else {
		treeEl.replaceChildren();
	}
}

async function runSearch() {
	const query = searchInput.value.trim();
	if (!query) {
		clearSearch();
		return;
	}
	try {
		if (libraryMode) {
			const hits = await invoke("search_library", { query, limit: 30 });
			renderLibrarySearchResults(hits);
		} else if (workMode) {
			if (legacyAgentView) await loadAgentWorks();
			else await loadWorks();
		} else {
			const hits = await invoke("search_index", { query, limit: 30 });
			renderSearchResults(hits);
		}
	} catch (error) {
		console.error(error);
	}
}

function renderSearchResults(hits) {
	treeEl.replaceChildren();
	if (!hits.length) {
		const tip = document.createElement("div");
		tip.className = "empty-tip";
		tip.textContent = "没有匹配的结果";
		treeEl.appendChild(tip);
		return;
	}
	const fragment = document.createDocumentFragment();
	hits.forEach((hit) => {
		const row = document.createElement("button");
		row.className = "tree-file search-hit";
		row.style.paddingLeft = "10px";
		const title = document.createElement("span");
		title.className = "hit-title";
		title.textContent = hit.title;
		const snippet = document.createElement("span");
		snippet.className = "hit-snippet";
		snippet.textContent = hit.snippet || hit.path;
		row.append(title, snippet);
		row.addEventListener("click", () => {
			const path = hit.path;
			clearSearch();
			openFile(path, basename(path), null);
		});
		fragment.appendChild(row);
	});
	treeEl.appendChild(fragment);
}

function formatWebSearchTime(value) {
	const date = new Date(value);
	if (Number.isNaN(date.getTime())) return value || "";
	return date.toLocaleString("zh-CN", {
		month: "numeric",
		day: "numeric",
		hour: "2-digit",
		minute: "2-digit",
	});
}

function webSearchDocumentLabel(uri) {
	if (!uri) return "未关联笔记";
	if (uri.startsWith("workspace://")) return `笔记：${uri.slice(12)}`;
	return `来源：${uri}`;
}

async function openWebSearchUrl(url, event) {
	event.preventDefault();
	try {
		if (typeof openUrl !== "function")
			throw new Error("Tauri opener API 不可用");
		await openUrl(url);
	} catch (error) {
		console.error("打开网页失败", error);
		markError("无法打开网页");
	}
}

async function toggleWebSearchResultLink(resultId) {
	const token = webSearchHistoryToken;
	const sourceUri = currentDocumentUri();
	if (!sourceUri) {
		markError("请先打开当前笔记");
		return;
	}
	const linked = linkedWebSearchResultIds.has(Number(resultId));
	try {
		await invoke(linked ? "unlink_search_result" : "link_search_result", {
			sourceUri,
			resultId: Number(resultId),
		});
		if (token !== webSearchHistoryToken) return;
		if (linked) linkedWebSearchResultIds.delete(Number(resultId));
		else linkedWebSearchResultIds.add(Number(resultId));
		renderWebSearchState();
	} catch (error) {
		if (token !== webSearchHistoryToken) return;
		console.error("更新网页搜索关联失败", error);
		markError("关联搜索结果失败");
	}
}

function renderWebSearchResultCard(result) {
	const card = document.createElement("article");
	card.className = `web-search-result${
		linkedWebSearchResultIds.has(Number(result.id)) ? " linked" : ""
	}`;
	const link = document.createElement("a");
	link.className = "web-search-hit";
	link.href = result.url;
	link.target = "_blank";
	link.rel = "noopener noreferrer";
	const title = document.createElement("span");
	title.className = "hit-title";
	title.textContent = result.title;
	const description = document.createElement("span");
	description.className = "hit-snippet";
	description.textContent = result.description || result.url;
	const meta = document.createElement("span");
	meta.className = "web-search-meta";
	let host = result.url;
	try {
		host = new URL(result.url).hostname;
	} catch (_) {}
	meta.textContent = result.age ? `${host} · ${result.age}` : host;
	link.append(title, description, meta);
	link.addEventListener(
		"click",
		(event) => void openWebSearchUrl(result.url, event),
	);
	card.appendChild(link);

	const actions = document.createElement("div");
	actions.className = "web-search-result-actions";
	const relation = document.createElement("button");
	relation.type = "button";
	relation.className = "related-card-pin web-search-link-button";
	const linked = linkedWebSearchResultIds.has(Number(result.id));
	relation.disabled = !currentDocumentUri();
	relation.textContent = linked ? "✓ 已关联当前笔记" : "关联当前笔记";
	relation.title = linked
		? "取消与当前笔记的关联"
		: "把这条网页搜索结果关联到当前笔记";
	relation.addEventListener("click", (event) => {
		event.preventDefault();
		event.stopPropagation();
		void toggleWebSearchResultLink(result.id);
	});
	actions.appendChild(relation);
	card.appendChild(actions);
	return card;
}

function renderWebSearchHistory() {
	webSearchStream.replaceChildren();
	renderWebSearchToolbar();
	const heading = document.createElement("div");
	heading.className = "web-search-heading";
	heading.textContent = `网页搜索历史 · ${webSearchHistory.length} 条`;
	webSearchStream.appendChild(heading);
	if (webSearchMessage) {
		const message = document.createElement("div");
		message.className = "empty-tip web-search-message";
		message.textContent = webSearchMessage;
		webSearchStream.appendChild(message);
	}
	if (!webSearchHistory.length) {
		if (!webSearchMessage) {
			const empty = document.createElement("div");
			empty.className = "empty-tip web-search-message";
			empty.textContent = "还没有网页搜索历史，选中文字后点“联网搜索”";
			webSearchStream.appendChild(empty);
		}
		return;
	}

	const fragment = document.createDocumentFragment();
	for (const history of webSearchHistory) {
		const card = document.createElement("article");
		card.className = "web-search-history";
		const expanded = expandedWebSearchIds.has(history.id);
		const toggle = document.createElement("button");
		toggle.type = "button";
		toggle.className = "web-search-history-toggle";
		toggle.setAttribute("aria-expanded", String(expanded));
		const marker = document.createElement("span");
		marker.className = "web-search-history-marker";
		marker.textContent = expanded ? "⌄" : "›";
		const query = document.createElement("strong");
		query.className = "web-search-history-query";
		query.textContent = history.query;
		query.title = history.query;
		const meta = document.createElement("span");
		meta.className = "web-search-history-meta";
		meta.textContent = `${history.results?.length || 0} 条结果 · ${formatWebSearchTime(history.createdAt)} · ${webSearchDocumentLabel(history.documentUri)}`;
		toggle.append(marker, query, meta);
		toggle.addEventListener("click", () => {
			if (expanded) expandedWebSearchIds.delete(history.id);
			else expandedWebSearchIds.add(history.id);
			renderWebSearchState();
		});
		card.appendChild(toggle);
		if (history.selectedText) {
			const quote = document.createElement("div");
			quote.className = "web-search-history-quote";
			quote.textContent = `选区：${history.selectedText}`;
			card.appendChild(quote);
		}
		if (expanded) {
			const results = document.createElement("div");
			results.className = "web-search-history-results";
			if (!history.results?.length) {
				const empty = document.createElement("div");
				empty.className = "empty-tip web-search-message";
				empty.textContent = "这次搜索没有返回网页";
				results.appendChild(empty);
			} else {
				for (const result of history.results)
					results.appendChild(renderWebSearchResultCard(result));
			}
			card.appendChild(results);
		}
		fragment.appendChild(card);
	}
	webSearchStream.appendChild(fragment);
}

function renderWebSearchMessage(message) {
	webSearchMessage = message;
	renderWebSearchHistory();
}

function renderWebSearchState() {
	renderWebSearchHistory();
}

function renderTree(nodes) {
	lastTreeNodes = nodes;
	treeEl.replaceChildren();
	const rootNode = {
		name: basename(rootPath),
		path: rootPath,
		is_dir: true,
		children: nodes,
	};
	treeEl.appendChild(treeNodeElement(rootNode, 0, true));
	if (!nodes.length) {
		const tip = document.createElement("div");
		tip.className = "empty-tip";
		tip.textContent = "这个文件夹里还没有 Markdown 文件";
		treeEl.appendChild(tip);
	}
}

function treeNodeElement(node, depth, forceExpanded = false) {
	if (node.is_dir) {
		const wrap = document.createElement("div");
		const normalizedDir = node.path.replaceAll("\\", "/").replace(/\/+$/, "");
		const normalizedFile = currentFile?.replaceAll("\\", "/");
		const expanded =
			forceExpanded || normalizedFile?.startsWith(`${normalizedDir}/`) || false;
		wrap.className = expanded ? "tree-dir-wrap" : "tree-dir-wrap collapsed";
		const button = document.createElement("button");
		button.className = "tree-dir";
		button.style.paddingLeft = `${12 + depth * 14}px`;
		const chevron = document.createElement("span");
		chevron.className = "chevron";
		chevron.textContent = expanded ? "⌄" : "›";
		const name = document.createElement("span");
		name.className = "node-name";
		name.textContent = node.name;
		button.append(chevron, name);
		const children = document.createElement("div");
		children.className = "tree-children";
		let childrenRendered = false;
		const renderChildren = () => {
			if (childrenRendered) return;
			const fragment = document.createDocumentFragment();
			node.children.forEach((child) =>
				fragment.appendChild(treeNodeElement(child, depth + 1)),
			);
			children.appendChild(fragment);
			childrenRendered = true;
		};
		if (expanded) renderChildren();
		button.addEventListener("click", () => {
			const collapsed = wrap.classList.toggle("collapsed");
			if (!collapsed) renderChildren();
			chevron.textContent = collapsed ? "›" : "⌄";
		});
		button.addEventListener("contextmenu", (event) => {
			event.preventDefault();
			openTreeContextMenu(
				{
					kind: node.path === rootPath ? "root" : "directory",
					path: node.path,
					name: node.name,
				},
				event.clientX,
				event.clientY,
			);
		});
		wrap.append(button, children);
		return wrap;
	}

	const button = document.createElement("button");
	button.className = "tree-file";
	button.style.paddingLeft = `${30 + depth * 14}px`;
	button.textContent = node.name.replace(/\.(md|markdown)$/i, "");
	button.title = node.path;
	if (node.path === currentFile) button.classList.add("active");
	button.addEventListener("click", () =>
		openFile(node.path, node.name, button),
	);
	button.addEventListener("contextmenu", (event) => {
		event.preventDefault();
		openTreeContextMenu(
			{ kind: "markdown", path: node.path, name: node.name },
			event.clientX,
			event.clientY,
		);
	});
	return button;
}

function basename(path) {
	const normalized = path.replace(/[\\/]+$/, "");
	return normalized.split(/[\\/]/).pop() || path;
}

function applyLayout() {
	sidebarWidth = Math.max(180, Math.min(420, sidebarWidth));
	splitRatio = Math.max(20, Math.min(80, splitRatio));
	annotateWidth = Math.max(300, Math.min(520, annotateWidth));
	document.documentElement.style.setProperty(
		"--sidebar-width",
		`${sidebarWidth}px`,
	);
	document.documentElement.style.setProperty(
		"--annotate-width",
		`${annotateWidth}px`,
	);
	editorPane.style.flexBasis = `${splitRatio}%`;
	readerPane.style.flexBasis = `${100 - splitRatio}%`;
	sidebar.classList.toggle("hidden", !sidebarVisible);
	sidebarHandle.classList.toggle("hidden", !sidebarVisible);
	annotatePanel.classList.toggle("hidden", !annotateVisible);
	annotatePanel.hidden = !annotateVisible;
	annotationButton.classList.toggle("active", annotateVisible);
	updateRightPanelHandle();
	// M3 Shell：Work Surface（工作标签下未进入 Markdown 时）占据中栏
	shell.dataset.mode = workSurfaceActive ? "work" : viewMode;
	workPane.classList.toggle("hidden", !workSurfaceActive);
	document.querySelectorAll("[data-view]").forEach((button) => {
		button.classList.toggle("active", button.dataset.view === viewMode);
	});
}

function documentViewKey() {
	const ref = currentDocumentRef();
	const key = ref ? documentRefKey(ref) : null;
	return key || "default";
}

/** 切换文档时恢复该文档自己的 写|双|读 模式（document-local）。 */
function loadViewModeForCurrentDocument() {
	viewMode =
		viewModesByDocument[documentViewKey()] ||
		localStorage.getItem("stillwrite.viewMode") ||
		"split";
	applyLayout();
}

function setViewMode(mode) {
	viewMode = mode;
	viewModesByDocument[documentViewKey()] = mode;
	localStorage.setItem(
		"stillwrite.viewModes",
		JSON.stringify(viewModesByDocument),
	);
	// 兼容旧默认：新文档未显式选择前的回退值
	localStorage.setItem("stillwrite.viewMode", mode);
	applyLayout();
	updateFormatToolbarState();
}

function setSidebarVisible(visible) {
	sidebarVisible = visible;
	localStorage.setItem("stillwrite.sidebarVisible", String(visible));
	applyLayout();
}

function bindResize(handle, getStart, onMove, onEnd) {
	handle.addEventListener("pointerdown", (event) => {
		event.preventDefault();
		const startX = event.clientX;
		const start = getStart();
		document.body.classList.add("resizing");
		handle.setPointerCapture(event.pointerId);
		const move = (e) => onMove(start, e.clientX - startX);
		const up = (e) => {
			handle.removeEventListener("pointermove", move);
			handle.removeEventListener("pointerup", up);
			document.body.classList.remove("resizing");
			onEnd();
			try {
				handle.releasePointerCapture(e.pointerId);
			} catch (_) {}
		};
		handle.addEventListener("pointermove", move);
		handle.addEventListener("pointerup", up);
	});
}

// ---------------------------------------------------------------------------
// 文件菜单（File / Recent / Workspace 子菜单）与文件树右键菜单。
// 菜单不持有业务状态：只投影 rootPath / currentFile / recentLocations，
// 启用矩阵等纯逻辑在 file-menu.js（StillwriteFileMenu）。
// ---------------------------------------------------------------------------

const FileMenuHelpers = window.StillwriteFileMenu;
let recentLocations = [];
let submenuHoverCloseTimer = null;
let pendingNewFolderParent = null;

function fileMenuContext() {
	return {
		hasWorkspace: Boolean(rootPath),
		currentFile,
		libraryDoc: currentLibraryDocument,
		agentDoc: currentAgentDocument,
	};
}

/// DOM 投影当前菜单启用矩阵；在每个文档/工作区状态变化点被
/// updateFormatToolbarState 带动刷新，打开菜单时也会再刷一次。
function updateFileMenuState() {
	const state = FileMenuHelpers.menuState(fileMenuContext());
	newFileItem.disabled = !state.newFile;
	openFolderItem.disabled = !state.open;
	openDocumentItem.disabled = !state.open;
	recentMenuTrigger.disabled = !state.recent;
	saveDocumentItem.disabled = !state.save;
	saveDocumentAsItem.disabled = !state.saveAs;
	workspaceMenuTrigger.disabled = !state.workspace;
	if (state.workspace === false) setFileMenuSubmenuOpen(workspaceMenuTrigger, workspaceMenu, false);
}

function setFileMenuOpen(open) {
	if (!open) {
		closeFileMenuSubmenus();
	} else {
		updateFileMenuState();
		renderRecentMenu(); // 先按缓存投影，异步刷新后再重绘
		void loadRecentLocations();
	}
	fileMenu.hidden = !open;
	fileMenuButton.setAttribute("aria-expanded", String(open));
	if (open) {
		const items = visibleEnabledMenuItems(fileMenu);
		items[0]?.focus();
	}
}

function anyFileSubmenuOpen() {
	return !recentMenu.hidden || !workspaceMenu.hidden;
}

function closeFileMenuSubmenus() {
	setFileMenuSubmenuOpen(recentMenuTrigger, recentMenu, false);
	setFileMenuSubmenuOpen(workspaceMenuTrigger, workspaceMenu, false);
}

function setFileMenuSubmenuOpen(trigger, menu, open) {
	if (open) {
		if (trigger.disabled) return;
		closeFileMenuSubmenus();
		positionSubmenu(menu);
		menu.hidden = false;
		trigger.setAttribute("aria-expanded", "true");
	} else if (!menu.hidden) {
		menu.hidden = true;
		trigger.setAttribute("aria-expanded", "false");
	}
}

/// 子菜单视口碰撞：右缘放不下翻到左侧，下缘放不下对齐底边。
function positionSubmenu(menu) {
	menu.classList.remove("open-left", "open-up");
	const wasHidden = menu.hidden;
	if (wasHidden) {
		menu.hidden = false;
		menu.style.visibility = "hidden";
	}
	const margin = 8;
	const rect = menu.getBoundingClientRect();
	if (rect.right + margin > window.innerWidth && rect.left - rect.width - margin >= 0) {
		menu.classList.add("open-left");
	}
	const adjusted = menu.getBoundingClientRect();
	if (adjusted.bottom + margin > window.innerHeight && adjusted.top - adjusted.height - margin >= 0) {
		menu.classList.add("open-up");
	}
	if (wasHidden) {
		menu.hidden = true;
		menu.style.visibility = "";
	}
}

/// 当前可见且可用的 menuitem（隐藏子菜单里的条目会被 offsetParent 过滤掉）。
function visibleEnabledMenuItems(menu) {
	return [...menu.querySelectorAll("button[role='menuitem']")].filter(
		(el) => !el.disabled && el.offsetParent !== null,
	);
}

async function loadRecentLocations() {
	try {
		recentLocations = (await invoke("list_recent_locations")) || [];
	} catch (error) {
		console.error("读取最近打开失败", error);
		recentLocations = [];
	}
	if (!fileMenu.hidden) renderRecentMenu();
}

/// renderRecentMenu(items)：只投影 recentLocations，不缓存第二份真值。
function renderRecentMenu() {
	const models = recentLocations.map(FileMenuHelpers.recentItemModel);
	recentMenu.replaceChildren();
	if (!models.length) {
		const empty = document.createElement("div");
		empty.className = "recent-menu-empty";
		empty.textContent = "暂无最近记录";
		recentMenu.appendChild(empty);
	} else {
		models.slice(0, 12).forEach((model) => {
			const button = document.createElement("button");
			button.type = "button";
			button.setAttribute("role", "menuitem");
			button.className = "recent-item";
			button.disabled = model.disabled;
			button.title = model.title;
			const line = document.createElement("span");
			line.className = "recent-item-line";
			const icon = document.createElement("span");
			icon.className = "recent-item-icon";
			icon.setAttribute("aria-hidden", "true");
			icon.textContent = model.icon;
			const label = document.createElement("span");
			label.className = "recent-item-label";
			label.textContent = model.displayLabel;
			line.append(icon, label);
			const path = document.createElement("span");
			path.className = "recent-item-path";
			path.textContent = model.path;
			button.append(line, path);
			button.addEventListener("click", () => {
				setFileMenuOpen(false);
				void openRecentLocation(model);
			});
			recentMenu.appendChild(button);
		});
	}
	const divider = document.createElement("div");
	divider.className = "menu-divider";
	divider.setAttribute("role", "separator");
	const clear = document.createElement("button");
	clear.type = "button";
	clear.setAttribute("role", "menuitem");
	clear.className = "recent-clear";
	clear.textContent = "清除最近记录";
	clear.disabled = !models.length;
	clear.addEventListener("click", async () => {
		setFileMenuOpen(false);
		try {
			await invoke("clear_recent_locations");
			recentLocations = [];
		} catch (error) {
			console.error("清除最近打开失败", error);
		}
	});
	recentMenu.append(divider, clear);
}

async function openRecentLocation(model) {
	try {
		if (agentBusy) await cancelAgentTurn();
		await saveCurrent();
		if (annotateDirty) await saveAnnotate();
		if (model.kind === "workspace") {
			const data = await invoke("open_recent_workspace", { path: model.path });
			await openWorkspaceData(data);
		} else {
			const data = await invoke("open_recent_document", { path: model.path });
			currentFile = data.path;
			currentAgentDocument = null;
			await setSidebarMode("workspace");
			await useWorkspace(data);
			showDocument(data.path, data.name, data.content, null);
		}
	} catch (error) {
		console.error(error);
		markError(backendErrorMessage(error, "打开失败：目标可能已不存在"));
	}
}

// ---- 文件树右键菜单：单个共享节点，运行时按 node kind 注入条目 ----

function openTreeContextMenu(context, x, y) {
	treeContextMenu.replaceChildren();
	const entries = [];
	if (context.kind === "markdown") {
		entries.push({
			label: "打开",
			action: () => void openFile(context.path, context.name, null),
		});
	} else {
		// root 与 directory 同一组动作；reveal 作用于该目录自身
		entries.push({
			label: "新建 Markdown",
			action: () => openNewFileDialogUnder(context.path),
		});
		entries.push({
			label: "新建文件夹",
			action: () => openNewFolderDialogUnder(context.path),
		});
	}
	entries.push({ separator: true });
	entries.push({
		label: "在文件管理器中显示",
		action: () => void revealPath(context.path),
	});
	for (const entry of entries) {
		if (entry.separator) {
			const divider = document.createElement("div");
			divider.className = "menu-divider";
			divider.setAttribute("role", "separator");
			treeContextMenu.appendChild(divider);
			continue;
		}
		const button = document.createElement("button");
		button.type = "button";
		button.setAttribute("role", "menuitem");
		button.textContent = entry.label;
		button.addEventListener("click", () => {
			closeTreeContextMenu();
			entry.action();
		});
		treeContextMenu.appendChild(button);
	}
	treeContextMenu.hidden = false;
	const rect = treeContextMenu.getBoundingClientRect();
	treeContextMenu.style.left = `${Math.max(
		4,
		Math.min(x, window.innerWidth - rect.width - 4),
	)}px`;
	treeContextMenu.style.top = `${Math.max(
		4,
		Math.min(y, window.innerHeight - rect.height - 4),
	)}px`;
	const first = treeContextMenu.querySelector("button");
	first?.focus();
}

function closeTreeContextMenu() {
	treeContextMenu.hidden = true;
}

/// 右键目录新建 Markdown：backend 只接受 root-relative 路径，
/// 对话框默认 untitled.md，子目录时预填 docs/research/untitled.md。
function openNewFileDialogUnder(directoryPath) {
	if (!rootPath) return;
	const prefix = relativeDirPrefix(directoryPath);
	newFileName.value = prefix ? `${prefix}/untitled.md` : "untitled.md";
	newFileDialog.showModal();
	requestAnimationFrame(() => newFileName.select());
}

function openNewFolderDialogUnder(directoryPath) {
	if (!rootPath) return;
	pendingNewFolderParent = directoryPath;
	newFolderName.value = "untitled";
	newFolderDialog.showModal();
	requestAnimationFrame(() => newFolderName.select());
}

async function createWorkspaceFolder(parentPath, name) {
	try {
		await invoke("create_workspace_folder", { parentPath, name });
		await refreshWorkspace();
	} catch (error) {
		console.error(error);
		markError(backendErrorMessage(error, "新建文件夹失败"));
	}
}

function relativeDirPrefix(directoryPath) {
	if (!rootPath || !directoryPath || samePath(directoryPath, rootPath)) return "";
	const normalizedDir = directoryPath.replaceAll("\\", "/").replace(/\/+$/, "");
	const normalizedRoot = rootPath.replaceAll("\\", "/").replace(/\/+$/, "");
	return normalizedDir.startsWith(`${normalizedRoot}/`)
		? normalizedDir.slice(normalizedRoot.length + 1)
		: "";
}

bindResize(
	sidebarHandle,
	() => sidebarWidth,
	(start, dx) => {
		sidebarWidth = Math.max(180, Math.min(420, start + dx));
		document.documentElement.style.setProperty(
			"--sidebar-width",
			`${sidebarWidth}px`,
		);
	},
	() => localStorage.setItem("stillwrite.sidebarWidth", String(sidebarWidth)),
);

bindResize(
	paneHandle,
	() => splitRatio,
	(start, dx) => {
		const width = panes.clientWidth || 1;
		splitRatio = Math.max(20, Math.min(80, start + (dx / width) * 100));
		editorPane.style.flexBasis = `${splitRatio}%`;
		readerPane.style.flexBasis = `${100 - splitRatio}%`;
	},
	() => localStorage.setItem("stillwrite.splitRatio", String(splitRatio)),
);

bindResize(
	annotateHandle,
	() => annotateWidth,
	(start, dx) => {
		annotateWidth = Math.max(300, Math.min(520, start - dx));
		document.documentElement.style.setProperty(
			"--annotate-width",
			`${annotateWidth}px`,
		);
	},
	() => localStorage.setItem("stillwrite.annotateWidth", String(annotateWidth)),
);

editor.addEventListener("input", onEditorContentChanged);

editor.addEventListener("select", () => setTimeout(showEditorSelectionAction));

editor.addEventListener("keydown", (event) => {
	if (editor.readOnly) return;
	if (event.key === "Tab") {
		event.preventDefault();
		const start = editor.selectionStart;
		const end = editor.selectionEnd;
		editor.setRangeText("  ", start, end, "end");
		editor.dispatchEvent(new Event("input"));
		return;
	}
	const mod = event.ctrlKey || event.metaKey;
	if (!mod) return;
	const key = event.key.toLowerCase();
	if (key === "b") {
		event.preventDefault();
		if (event.shiftKey) {
			setSidebarVisible(!sidebarVisible);
		} else {
			applyFormatCommand("bold");
		}
		return;
	}
	if (key === "i") {
		event.preventDefault();
		applyFormatCommand("italic");
		return;
	}
	if (key === "k") {
		event.preventDefault();
		applyFormatCommand("link");
		return;
	}
	if (event.altKey && /^[1-6]$/.test(event.key)) {
		event.preventDefault();
		applyFormatCommand("heading", { level: Number(event.key) });
	}
});

document
	.querySelector("#openFolder")
	.addEventListener("click", chooseWorkspace);
document
	.querySelector("#openDocument")
	.addEventListener("click", chooseDocument);
document.querySelector("#refreshTree").addEventListener("click", refreshTree);
workspaceTab.addEventListener("click", () => void setSidebarMode("workspace"));
libraryTab.addEventListener("click", () => void setSidebarMode("library"));
workTabRef.addEventListener("click", () => void setSidebarMode("work"));
legacyAgentWorksToggle.addEventListener(
	"click",
	() => void toggleLegacyAgentWorks(),
);
workContextViewButton.addEventListener("click", () =>
	setSupportView("work-context"),
);
workEvidenceViewButton.addEventListener("click", () =>
	setSupportView("work-evidence"),
);
addLibrarySourceButton.addEventListener("click", () => {
	setSourceMenuOpen(sourceMenu.hidden);
});
addLibrarySourceMenuItem.addEventListener("click", () => {
	setSourceMenuOpen(false);
	void addLibrarySource();
});
addFeedMenuItem.addEventListener("click", () => {
	setSourceMenuOpen(false);
	addFeedDialog.showModal();
	requestAnimationFrame(() => addFeedUrl.focus());
});
importOpmlMenuItem.addEventListener("click", () => {
	setSourceMenuOpen(false);
	void importOpml();
});
addFeedForm.addEventListener("submit", submitAddFeed);
cancelAddFeed.addEventListener("click", () => addFeedDialog.close());
document.addEventListener("pointerdown", (event) => {
	if (
		!sourceMenu.hidden &&
		!event.target.closest("#addLibrarySource") &&
		!sourceMenu.contains(event.target)
	)
		setSourceMenuOpen(false);
});
newAgentWorkButton.addEventListener(
	"click",
	() => void beginAgentQuestion(null, { allowEmpty: true }),
);
clearWorksetButton.addEventListener("click", () => {
	citationBasket.clear();
	renderCitationSummary();
	if (libraryMode && searchInput.value.trim()) void runSearch();
});
document.querySelector("#saveDocument").addEventListener("click", saveCurrent);
saveDocumentAsItem.addEventListener("click", () => void saveDocumentAs());
fileMenuButton.addEventListener("click", () =>
	setFileMenuOpen(fileMenu.hidden),
);
fileMenu.addEventListener("click", (event) => {
	const button = event.target.closest("button");
	if (!button) return;
	// 子菜单触发项：点击始终打开（不 toggle）——hover 可能已把它打开，
	// 此时点击不应把刚打开的子菜单立刻关掉；关闭走 hover 离开 / Esc / Left。
	if (button === recentMenuTrigger) {
		setFileMenuSubmenuOpen(recentMenuTrigger, recentMenu, true);
		return;
	}
	if (button === workspaceMenuTrigger) {
		setFileMenuSubmenuOpen(workspaceMenuTrigger, workspaceMenu, true);
		return;
	}
	// 普通项（含子菜单内条目）：各自 handler 执行动作，这里统一收口关闭
	if (!button.disabled) setFileMenuOpen(false);
});
// hover 可以打开子菜单，但不只依赖 hover；离开不立即关闭（跨 6px gap 不闪烁）
function bindFileSubmenuHover(wrap, trigger, menu) {
	wrap.addEventListener("pointerenter", () => {
		clearTimeout(submenuHoverCloseTimer);
		if (!fileMenu.hidden && !trigger.disabled) {
			setFileMenuSubmenuOpen(trigger, menu, true);
		}
	});
	wrap.addEventListener("pointerleave", () => {
		clearTimeout(submenuHoverCloseTimer);
		submenuHoverCloseTimer = setTimeout(() => {
			if (!wrap.matches(":hover") && !menu.matches(":hover")) {
				setFileMenuSubmenuOpen(trigger, menu, false);
			}
		}, 220);
	});
}
bindFileSubmenuHover(
	document.querySelector("#recentMenuWrap"),
	recentMenuTrigger,
	recentMenu,
);
bindFileSubmenuHover(
	document.querySelector("#workspaceMenuWrap"),
	workspaceMenuTrigger,
	workspaceMenu,
);
// 键盘导航：Down/Up 在当前菜单移动，Right 进子菜单，Left 回父项，Home/End 首尾
fileMenuRoot.addEventListener("keydown", (event) => {
	const key = event.key;
	const focusTarget = event.target instanceof Element ? event.target : null;
	if (key === "ArrowDown" || key === "ArrowUp") {
		event.preventDefault();
		if (fileMenu.hidden) {
			setFileMenuOpen(true);
			return;
		}
		const menu = focusTarget?.closest("[role='menu']");
		if (!menu) {
			visibleEnabledMenuItems(fileMenu)[0]?.focus();
			return;
		}
		navigateMenuItems(menu, focusTarget, key === "ArrowDown" ? 1 : -1);
		return;
	}
	if (key === "ArrowRight") {
		if (focusTarget === recentMenuTrigger && recentMenu.hidden) {
			event.preventDefault();
			setFileMenuSubmenuOpen(recentMenuTrigger, recentMenu, true);
			visibleEnabledMenuItems(recentMenu)[0]?.focus();
		} else if (
			focusTarget === workspaceMenuTrigger &&
			workspaceMenu.hidden
		) {
			event.preventDefault();
			setFileMenuSubmenuOpen(workspaceMenuTrigger, workspaceMenu, true);
			visibleEnabledMenuItems(workspaceMenu)[0]?.focus();
		}
		return;
	}
	if (key === "ArrowLeft") {
		const menu = focusTarget?.closest("[role='menu']");
		if (menu === recentMenu || menu === workspaceMenu) {
			event.preventDefault();
			const trigger = menu === recentMenu ? recentMenuTrigger : workspaceMenuTrigger;
			setFileMenuSubmenuOpen(trigger, menu, false);
			trigger.focus();
		}
		return;
	}
	if (key === "Home" || key === "End") {
		const menu = focusTarget?.closest("[role='menu']");
		if (!menu) return;
		event.preventDefault();
		navigateMenuItems(menu, null, key === "Home" ? "first" : "end");
	}
});
function navigateMenuItems(menu, currentEl, direction) {
	const items = visibleEnabledMenuItems(menu);
	const models = items.map((el) => ({ disabled: el.disabled, hidden: el.hidden }));
	if (direction === "first" || direction === "end") {
		const edge = FileMenuHelpers.edgeEnabledMenuItem(
			models,
			direction === "first" ? "first" : "end",
		);
		if (edge >= 0) items[edge].focus();
		return;
	}
	const current = currentEl ? items.indexOf(currentEl) : -1;
	const next = FileMenuHelpers.nextEnabledMenuItem(models, current, direction);
	if (next >= 0) items[next].focus();
}
workspaceRevealItem.addEventListener("click", () => {
	if (rootPath) void revealPath(rootPath);
});
workspaceRefreshItem.addEventListener("click", () => void refreshWorkspace());
workspaceAggregateItem.addEventListener("click", () => {
	if (!annotateVisible) setAnnotateVisible(true);
	void doAggregateAnnotations();
});
workspaceCloseItem.addEventListener("click", () => void closeWorkspace());
document.addEventListener("pointerdown", (event) => {
	if (!fileMenuRoot.contains(event.target)) setFileMenuOpen(false);
	if (
		!treeContextMenu.hidden &&
		!treeContextMenu.contains(event.target)
	)
		closeTreeContextMenu();
});
newFolderForm.addEventListener("submit", (event) => {
	event.preventDefault();
	const name = newFolderName.value.trim();
	if (!name || !pendingNewFolderParent) return;
	newFolderDialog.close();
	void createWorkspaceFolder(pendingNewFolderParent, name);
});
cancelNewFolderButton.addEventListener("click", () => newFolderDialog.close());
document
	.querySelector("#toggleSidebar")
	.addEventListener("click", () => setSidebarVisible(!sidebarVisible));
document.querySelector("#newFile").addEventListener("click", async () => {
	if (!rootPath) {
		await chooseWorkspace();
		if (!rootPath) return;
	}
	newFileName.value = "untitled.md";
	newFileDialog.showModal();
	requestAnimationFrame(() => newFileName.select());
});

document.querySelector("#syncButton").addEventListener("click", doSync);
settingsButton.addEventListener("click", () => void openSettings());
settingsForm.addEventListener("submit", (event) => {
	event.preventDefault();
	void saveSettings();
});
cancelSettingsButton.addEventListener("click", () => settingsDialog.close());
clearBraveApiKeyButton.addEventListener("click", () => void clearBraveApiKey());
annotationViewButton.addEventListener("click", () =>
	setSupportView("annotation"),
);
relatedViewButton.addEventListener("click", () => setSupportView("related"));
webSearchViewButton.addEventListener("click", () =>
	setSupportView("web-search"),
);
agentAskForm.addEventListener("submit", submitAgentQuestion);
agentAskCancel.addEventListener("click", () => {
	pendingAgentRequest = null;
	agentAskDialog.close();
});
annotationButton.addEventListener("click", () => {
	const hasSelection = editor.selectionStart !== editor.selectionEnd;
	if (hasSelection) beginAnnotation(captureEditorAnnotation());
	else setAnnotateVisible(!annotateVisible);
});
closeAnnotate.addEventListener("click", () => setAnnotateVisible(false));
newAnnotationButton.addEventListener("click", () => beginAnnotation());
addAnnotationButton.addEventListener("click", addPendingAnnotation);
cancelAnnotationButton.addEventListener("click", cancelAnnotation);
aggregateButton.addEventListener("click", doAggregateAnnotations);
annotationDraft.addEventListener("input", () =>
	autosizeAnnotationEditor(annotationDraft),
);
annotationDraft.addEventListener("keydown", (event) => {
	if (event.key === "Tab") {
		event.preventDefault();
		const start = annotationDraft.selectionStart;
		const end = annotationDraft.selectionEnd;
		annotationDraft.setRangeText("  ", start, end, "end");
		autosizeAnnotationEditor(annotationDraft);
	}
	if ((event.ctrlKey || event.metaKey) && event.key === "Enter") {
		event.preventDefault();
		addPendingAnnotation();
	}
});
previewEl.addEventListener("mouseup", () =>
	setTimeout(showPreviewSelectionAction),
);
previewEl.addEventListener("click", (event) => {
	const target =
		event.target instanceof Element
			? event.target
			: event.target?.parentElement;
	const anchor = target?.closest?.("a[data-document-path]");
	if (!anchor) return;
	event.preventDefault();
	event.stopPropagation();
	const path = anchor.dataset.documentPath;
	if (path) void openFile(path, basename(path), null);
});
selectionAnnotateButton.addEventListener("pointerdown", (event) => {
	event.preventDefault();
});
selectionAgentButton.addEventListener("pointerdown", (event) => {
	event.preventDefault();
});
selectionSearchButton.addEventListener("pointerdown", (event) => {
	event.preventDefault();
});
selectionRelatedButton.addEventListener("pointerdown", (event) => {
	event.preventDefault();
});
selectionAnnotateButton.addEventListener("click", () => {
	const range = pendingPreviewSelection;
	hideSelectionAnnotate();
	if (range) beginAnnotation(range);
});
selectionAgentButton.addEventListener("click", () => {
	const range = pendingPreviewSelection;
	hideSelectionAnnotate();
	if (range) void beginAgentQuestion(range);
});
selectionSearchButton.addEventListener("click", () => {
	const range = pendingPreviewSelection;
	hideSelectionAnnotate();
	if (range) void searchWebFromSelection(range);
});
selectionRelatedButton.addEventListener("click", () => {
	const range = pendingPreviewSelection;
	hideSelectionAnnotate();
	if (range) addRelatedSupplement(range.quote);
});
document.addEventListener("pointerdown", (event) => {
	if (
		!selectionActions.hidden &&
		!selectionActions.contains(event.target) &&
		!previewEl.contains(event.target)
	)
		hideSelectionAnnotate();
});
searchInput.addEventListener("input", () => {
	clearTimeout(searchTimer);
	searchTimer = setTimeout(runSearch, 250);
});
searchInput.addEventListener("keydown", (event) => {
	if (event.key === "Escape") {
		event.preventDefault();
		clearSearch();
		searchInput.blur();
	}
});

document.querySelectorAll("[data-view]").forEach((button) => {
	button.addEventListener("click", () => setViewMode(button.dataset.view));
});

newFileForm.addEventListener("submit", (event) => {
	event.preventDefault();
	let requested = newFileName.value.trim();
	if (!requested) return;
	if (!/\.(md|markdown)$/i.test(requested)) requested += ".md";
	newFileDialog.close();
	createFile(requested);
});

document.querySelector("#cancelNewFile").addEventListener("click", () => {
	newFileDialog.close();
});

window.addEventListener("keydown", (event) => {
	if (event.key === "Escape") {
		if (headingMenuOpen) {
			event.preventDefault();
			closeHeadingMenu();
			return;
		}
		if (!treeContextMenu.hidden) {
			event.preventDefault();
			closeTreeContextMenu();
			return;
		}
		if (!fileMenu.hidden) {
			event.preventDefault();
			// Esc 先关 submenu，再关 File menu
			if (anyFileSubmenuOpen()) {
				const openMenu = !recentMenu.hidden ? recentMenu : workspaceMenu;
				const trigger = openMenu === recentMenu ? recentMenuTrigger : workspaceMenuTrigger;
				setFileMenuSubmenuOpen(trigger, openMenu, false);
				trigger.focus();
			} else {
				setFileMenuOpen(false);
				fileMenuButton.focus();
			}
			return;
		}
	}
	const mod = event.ctrlKey || event.metaKey;
	if (!mod) return;
	const key = event.key.toLowerCase();
	if (key === "o") {
		event.preventDefault();
		if (event.shiftKey) chooseDocument();
		else chooseWorkspace();
	}
	if (key === "s") {
		event.preventDefault();
		if (event.shiftKey) void saveDocumentAs();
		else void saveCurrent();
	}
	if (key === "n") {
		event.preventDefault();
		document.querySelector("#newFile").click();
	}
	if (key === "r") {
		event.preventDefault();
		refreshTree();
	}
	if (key === "b") {
		event.preventDefault();
		// 编辑器聚焦时 Ctrl+B/Ctrl+Shift+B 由编辑器 keydown 处理（粗体/侧栏）
		if (document.activeElement === editor) return;
		setSidebarVisible(!sidebarVisible);
	}
	if (key === "m" && event.shiftKey) {
		event.preventDefault();
		beginAnnotation();
	}
	if (!event.altKey && key === "1") {
		event.preventDefault();
		setViewMode("editor");
	}
	if (!event.altKey && key === "2") {
		event.preventDefault();
		setViewMode("split");
	}
	if (!event.altKey && key === "3") {
		event.preventDefault();
		setViewMode("reader");
	}
});

applyLayout();
updateFormatToolbarState();
void installAgentEventListener().catch((error) => {
	console.warn("Cannot subscribe to Agent events", error);
});
// M3：默认启动进入 Work Home
void setSidebarMode("work");
restoreWorkspace();
