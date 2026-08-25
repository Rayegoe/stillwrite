const invoke = window.__TAURI__.core.invoke;

const shell = document.querySelector("#shell");
const sidebar = document.querySelector("#sidebar");
const sidebarHandle = document.querySelector("#sidebarHandle");
const paneHandle = document.querySelector("#paneHandle");
const panes = document.querySelector("#panes");
const editorPane = document.querySelector("#editorPane");
const readerPane = document.querySelector("#readerPane");
const editor = document.querySelector("#editor");
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
const fileMenuRoot = document.querySelector("#fileMenuRoot");
const fileMenuButton = document.querySelector("#fileMenuButton");
const fileMenu = document.querySelector("#fileMenu");
const annotationButton = document.querySelector("#annotationButton");
const annotatePanel = document.querySelector("#annotatePanel");
const annotateHandle = document.querySelector("#annotateHandle");
const closeAnnotate = document.querySelector("#closeAnnotate");
const aggregateButton = document.querySelector("#aggregateButton");
const aggregateMenu = document.querySelector("#aggregateMenu");
const annotateDocName = document.querySelector("#annotateDocName");
const annotateDocPath = document.querySelector("#annotateDocPath");
const annotateCount = document.querySelector("#annotateCount");
const annotateHint = document.querySelector("#annotateHint");
const annotateStream = document.querySelector("#annotateStream");
const annotationList = document.querySelector("#annotationList");
const annotationEmpty = document.querySelector("#annotationEmpty");
const annotateComposer = document.querySelector("#annotateComposer");
const annotationDraftQuote = document.querySelector("#annotationDraftQuote");
const annotationDraft = document.querySelector("#annotationDraft");
const newAnnotationButton = document.querySelector("#newAnnotation");
const addAnnotationButton = document.querySelector("#addAnnotation");
const cancelAnnotationButton = document.querySelector("#cancelAnnotation");
const selectionAnnotateButton = document.querySelector("#selectionAnnotate");
const annotateSaveState = document.querySelector("#annotateSaveState");
const annotateFoot = document.querySelector("#annotateFoot");
const AnnotationCodec = window.StillwriteAnnotations;
const DocumentLinks = window.StillwriteDocumentLinks;
const agentButton = document.querySelector("#agentButton");
const agentPanel = document.querySelector("#agentPanel");
const closeAgent = document.querySelector("#closeAgent");
const agentState = document.querySelector("#agentState");
const agentTranscript = document.querySelector("#agentTranscript");
const agentEvidence = document.querySelector("#agentEvidence");
const agentPrompt = document.querySelector("#agentPrompt");
const agentSend = document.querySelector("#agentSend");
const agentCancel = document.querySelector("#agentCancel");

let rootPath = localStorage.getItem("stillwrite.rootPath");
let currentFile = null;
let saveTimer = null;
let sidebarWidth = Number(
	localStorage.getItem("stillwrite.sidebarWidth") || 248,
);
let splitRatio = Number(localStorage.getItem("stillwrite.splitRatio") || 50);
let sidebarVisible =
	localStorage.getItem("stillwrite.sidebarVisible") !== "false";
let viewMode = localStorage.getItem("stillwrite.viewMode") || "split";
let loadToken = 0;

let annotateVisible =
	localStorage.getItem("stillwrite.annotateVisible") === "true";
let annotateWidth = Number(
	localStorage.getItem("stillwrite.annotateWidth") || 360,
);
let annotationItems = []; // 当前文档的结构化批注
let annotateLoadedDoc = null; // 当前加载了批注的文档路径
let annotateDirty = false;
let annotateTimer = null;
let activeAnnotationId = null;
let pendingAnnotation = null;
let pendingPreviewSelection = null;

const DEFAULT_REMOTE = "user@example.invalid:~/stillwrite.git";
let autoSync = false; // 首次手动同步成功后开启自动同步
let syncTimer = null;
let searchTimer = null;
let previewTimer = null;
let lastTreeNodes = [];
let documentLinkIndex = DocumentLinks.buildIndex([], rootPath);
const agentSessionId =
	globalThis.crypto?.randomUUID?.() ||
	`gui-${Date.now()}-${Math.random().toString(36).slice(2)}`;
let agentBusy = false;
let agentProbed = false;
let agentCancelRequested = false;

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
]);

function escapeHtml(value) {
	return value
		.replaceAll("&", "&amp;")
		.replaceAll("<", "&lt;")
		.replaceAll(">", "&gt;")
		.replaceAll('"', "&quot;")
		.replaceAll("'", "&#039;");
}

function setAgentState(text, kind = "") {
	agentState.textContent = text;
	agentState.classList.remove("ok", "busy", "error");
	if (kind) agentState.classList.add(kind);
}

function appendAgentMessage(role, text) {
	agentTranscript.querySelector(".agent-empty")?.remove();
	const item = document.createElement("article");
	item.className = `agent-message ${role}`;
	const label = document.createElement("div");
	label.className = "agent-message-role";
	label.textContent = role === "user" ? "YOU" : role === "agent" ? "AGENT" : "SYSTEM";
	const body = document.createElement("div");
	body.className = "agent-message-body";
	body.textContent = text;
	item.append(label, body);
	agentTranscript.append(item);
	agentTranscript.scrollTop = agentTranscript.scrollHeight;
}

function showAgentEvidence(response) {
	agentEvidence.hidden = false;
	agentEvidence.replaceChildren();
	const session = document.createElement("code");
	session.textContent = `conversation ${response.conversationId}`;
	const run = document.createElement("code");
	run.textContent = `run ${response.runId || "—"}`;
	const receipt = document.createElement("code");
	receipt.textContent = `receipt ${response.receiptRef || "—"}`;
	agentEvidence.append(session, run, receipt);
}

async function probeAgent() {
	if (agentProbed) return;
	setAgentState("正在探测…", "busy");
	try {
		const result = await invoke("agent_probe");
		agentProbed = true;
		setAgentState(`${result.profile} 已连接`, "ok");
		agentState.title = result.launcher;
	} catch (error) {
		setAgentState("不可用", "error");
		agentState.title = String(error);
	}
}

async function setAgentPanelVisible(visible) {
	agentPanel.hidden = !visible;
	agentButton.classList.toggle("active", visible);
	if (visible) {
		await probeAgent();
		agentPrompt.focus();
	}
}

async function sendAgentTurn() {
	if (agentBusy) return;
	if (!rootPath) {
		await chooseWorkspace();
		if (!rootPath) return;
	}
	const prompt = agentPrompt.value.trim();
	if (!prompt) return;
	agentBusy = true;
	agentCancelRequested = false;
	agentSend.disabled = true;
	agentCancel.disabled = false;
	setAgentState("Runtime 运行中…", "busy");
	appendAgentMessage("user", prompt);
	try {
		const response = await invoke("agent_turn", {
			input: {
				prompt,
				sessionId: agentSessionId,
				messageId: globalThis.crypto?.randomUUID?.() || `msg-${Date.now()}`,
			},
		});
		appendAgentMessage(response.status === "error" ? "system" : "agent", response.text);
		showAgentEvidence(response);
		agentPrompt.value = "";
		setAgentState(
			response.status === "paused" ? "运行已暂停" : `已返回 · ${response.status}`,
			response.status === "error" ? "error" : "ok",
		);
	} catch (error) {
		if (agentCancelRequested) {
			setAgentState("已停止等待此本地进程");
			appendAgentMessage("system", "已停止等待此本地进程；这不表示外部任务或效果已撤销。");
		} else {
			setAgentState("调用失败", "error");
			appendAgentMessage("system", String(error));
		}
	} finally {
		agentBusy = false;
		agentSend.disabled = false;
		agentCancel.disabled = true;
		agentCancelRequested = false;
		agentPrompt.focus();
	}
}

async function cancelAgentTurn() {
	if (!agentBusy) return;
	try {
		agentCancelRequested = await invoke("agent_cancel");
		if (agentCancelRequested) setAgentState("正在停止本地进程…", "busy");
	} catch (error) {
		setAgentState("停止失败", "error");
		appendAgentMessage("system", String(error));
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
		html.push(`<p>${paragraph.map((line) => renderInline(line)).join("<br>")}</p>`);
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

	for (const line of lines) {
		// 批注侧车中的结构标记是 Markdown 注释，阅读时不应显示。
		if (/^<!-- \/?stillwrite-(?:annotations|annotation|quote)/.test(line.trim()))
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

		const ul = line.match(/^\s*[-+*]\s+(.+)$/);
		const ol = line.match(/^\s*\d+[.)]\s+(.+)$/);
		if (ul || ol) {
			flushParagraph();
			const wanted = ul ? "ul" : "ol";
			if (listType !== wanted) {
				closeList();
				html.push(`<${wanted}>`);
				listType = wanted;
			}
			html.push(`<li>${renderInline((ul || ol)[1])}</li>`);
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
			if (name.startsWith("on")) el.removeAttribute(attr.name);
			else if (name === "href" && /^\s*javascript:/i.test(attr.value))
				attr.value = "#";
			else if (
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
		const segments = DocumentLinks.segmentText(textNode.data, documentLinkIndex);
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

function updatePreview() {
	if (previewTimer) {
		clearTimeout(previewTimer);
		previewTimer = null;
	}
	const doc = sanitizeHtml(renderMarkdown(editor.value));
	preparePreviewLinks(doc.body);
	previewEl.replaceChildren(...doc.body.childNodes);
	renderAnnotationAnchors();
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
	if (!currentFile) return;
	if (saveTimer) clearTimeout(saveTimer);
	saveTimer = setTimeout(saveCurrent, 650);
}

async function saveCurrent() {
	if (!currentFile) return;
	if (saveTimer) {
		clearTimeout(saveTimer);
		saveTimer = null;
	}
	try {
		await invoke("write_markdown", {
			path: currentFile,
			content: editor.value,
		});
		markSaved();
		if (autoSync) scheduleAutoSync();
	} catch (error) {
		console.error(error);
		markError("保存失败");
	}
}

async function useWorkspace(data) {
	if (!data) return;
	rootPath = data.root;
	localStorage.setItem("stillwrite.rootPath", rootPath);
	workspaceNameEl.textContent = basename(rootPath);
	workspaceNameEl.title = rootPath;
	lastTreeNodes = data.nodes;
	documentLinkIndex = DocumentLinks.buildIndex(data.nodes, rootPath);
	renderTree(data.nodes);
}

async function chooseWorkspace() {
	try {
		// Save against the current workspace before Rust switches the active root.
		await saveCurrent();
		if (annotateDirty) await saveAnnotate();
		const data = await invoke("choose_workspace");
		if (!data) return;
		currentFile = null;
		editor.value = "";
		updatePreview();
		documentTitleEl.textContent = "Stillwrite";
		await useWorkspace(data);
		loadAnnotationPanel();
	} catch (error) {
		console.error(error);
		markError("目录打开失败");
	}
}

async function chooseDocument() {
	try {
		await saveCurrent();
		if (annotateDirty) await saveAnnotate();
		const data = await invoke("choose_document");
		if (!data) return;
		currentFile = data.path;
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
	if (!rootPath) return;
	try {
		const data = await invoke("set_workspace", { path: rootPath });
		await useWorkspace(data);
	} catch (error) {
		console.error(error);
		markError("刷新失败");
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

function showDocument(path, name, text, row) {
	currentFile = path;
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
	loadAnnotationPanel();
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
	annotationEmpty.hidden = annotationItems.length > 0 || !annotateComposer.hidden;
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
	previewEl.querySelectorAll(".annotation-marker").forEach((node) => node.remove());
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
	if (annotateLoadedDoc !== currentFile) return;
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
	const card = annotationList.querySelector(`[data-annotation-id="${CSS.escape(id)}"]`);
	card?.scrollIntoView({ block: "nearest", behavior: "smooth" });
	if (!revealSource) return;
	const marker = previewEl.querySelector(`.annotation-marker[data-annotation-id="${CSS.escape(id)}"]`);
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

async function beginAnnotation(range = null) {
	if (!currentFile || !isAnnotatablePath(currentFile)) {
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
	if (annotateLoadedDoc !== currentFile) await loadAnnotationPanel();
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
	const requestedFile = currentFile;
	if (!requestedFile) {
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
		const data = await invoke("read_annotation", { docPath: requestedFile });
		if (requestedFile !== currentFile) return;
		annotationItems = AnnotationCodec.parse(data.body || "", data.updated_at || "");
		annotateLoadedDoc = requestedFile;
		activeAnnotationId = null;
		annotateDocName.textContent = basename(requestedFile).replace(
			/\.(md|markdown)$/i,
			"",
		);
		annotateDocPath.textContent = relativeDocumentPath(requestedFile);
		annotateDocPath.title = requestedFile;
		annotateDirty = false;
		cancelAnnotation();
		renderAnnotationPanel();
		renderAnnotationAnchors();
		annotateSaveState.textContent = annotationItems.length ? "已保存" : "";
		annotateSaveState.classList.remove("dirty", "error");
		annotateFoot.textContent = "批注保存在「批注/」，随文档同步。";
	} catch (error) {
		console.error(error);
		annotationItems = [];
		annotateLoadedDoc = requestedFile;
		activeAnnotationId = null;
		cancelAnnotation();
		renderAnnotationPanel();
		renderAnnotationAnchors();
		annotateFoot.textContent = String(error).includes("不能再写批注")
			? "批注文件本身不能再批注"
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
	if (!currentFile) return;
	if (annotateTimer) clearTimeout(annotateTimer);
	annotateTimer = setTimeout(saveAnnotate, 650);
}

async function saveAnnotate() {
	if (!currentFile || annotateLoadedDoc !== currentFile) return;
	if (annotateTimer) {
		clearTimeout(annotateTimer);
		annotateTimer = null;
	}
	const body = AnnotationCodec.serialize(annotationItems);
	try {
		await invoke("save_annotation", { docPath: currentFile, body });
		annotateSaveState.textContent = annotationItems.length ? "已保存" : "";
		annotateSaveState.classList.remove("dirty", "error");
		annotateDirty = false;
		if (autoSync) scheduleAutoSync();
	} catch (error) {
		console.error(error);
		annotateSaveState.textContent = "保存失败";
		annotateSaveState.classList.add("error");
	}
}

function hideSelectionAnnotate() {
	selectionAnnotateButton.hidden = true;
	pendingPreviewSelection = null;
}

function sourceRangeForPreviewSelection(quote) {
	const exact = editor.value.indexOf(quote);
	if (exact >= 0)
		return { kind: "selection", start: exact, end: exact + quote.length, quote };
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
	const rect = range.getBoundingClientRect();
	selectionAnnotateButton.style.left = `${Math.min(window.innerWidth - 82, Math.max(8, rect.left + rect.width / 2 - 34))}px`;
	selectionAnnotateButton.style.top = `${Math.max(8, rect.top - 40)}px`;
	selectionAnnotateButton.hidden = false;
}

// 源文档是否允许批注（批注文件与汇总文件自身不能再批注）
function isAnnotatablePath(path) {
	if (!path) return false;
	const normalized = path.replaceAll("\\", "/");
	return !(
		normalized.endsWith("/批注汇总.md") ||
		normalized.includes("/批注/")
	);
}

function samePath(a, b) {
	return a.replaceAll("\\", "/") === b.replaceAll("\\", "/");
}

async function doAggregateAnnotations() {
	if (!rootPath) return;
	// 先落盘待保存的批注：刚打完字就点汇总时，650ms 防抖可能还没触发
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

function setAnnotateVisible(visible) {
	annotateVisible = visible;
	localStorage.setItem("stillwrite.annotateVisible", String(visible));
	annotationButton.classList.toggle("active", visible);
	annotatePanel.hidden = !visible;
	annotatePanel.classList.toggle("hidden", !visible);
	annotateHandle.classList.toggle("hidden", !visible);
	document.documentElement.style.setProperty(
		"--annotate-width",
		`${annotateWidth}px`,
	);
	if (visible && annotateLoadedDoc !== currentFile) loadAnnotationPanel();
}

function clearSearch() {
	searchInput.value = "";
	if (lastTreeNodes.length) renderTree(lastTreeNodes);
	else treeEl.replaceChildren();
}

async function runSearch() {
	const query = searchInput.value.trim();
	if (!query) {
		clearSearch();
		return;
	}
	try {
		const hits = await invoke("search_index", { query, limit: 30 });
		renderSearchResults(hits);
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
	annotateHandle.classList.toggle("hidden", !annotateVisible);
	annotationButton.classList.toggle("active", annotateVisible);
	shell.dataset.mode = viewMode;
	document.querySelectorAll("[data-view]").forEach((button) => {
		button.classList.toggle("active", button.dataset.view === viewMode);
	});
}

function setViewMode(mode) {
	viewMode = mode;
	localStorage.setItem("stillwrite.viewMode", mode);
	applyLayout();
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

function setFileMenuOpen(open) {
	fileMenu.hidden = !open;
	fileMenuButton.setAttribute("aria-expanded", String(open));
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

editor.addEventListener("input", () => {
	schedulePreview();
	markDirty();
	scheduleSave();
});

editor.addEventListener("keydown", (event) => {
	if (event.key === "Tab") {
		event.preventDefault();
		const start = editor.selectionStart;
		const end = editor.selectionEnd;
		editor.setRangeText("  ", start, end, "end");
		editor.dispatchEvent(new Event("input"));
	}
});

document
	.querySelector("#openFolder")
	.addEventListener("click", chooseWorkspace);
document
	.querySelector("#openDocument")
	.addEventListener("click", chooseDocument);
document.querySelector("#refreshTree").addEventListener("click", refreshTree);
document.querySelector("#refreshMenu").addEventListener("click", refreshTree);
document.querySelector("#saveDocument").addEventListener("click", saveCurrent);
fileMenuButton.addEventListener("click", () => setFileMenuOpen(fileMenu.hidden));
fileMenu.addEventListener("click", (event) => {
	if (event.target.closest("button")) setFileMenuOpen(false);
});
document.addEventListener("pointerdown", (event) => {
	if (!fileMenuRoot.contains(event.target)) setFileMenuOpen(false);
});
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
agentButton.addEventListener("click", () => setAgentPanelVisible(agentPanel.hidden));
closeAgent.addEventListener("click", () => setAgentPanelVisible(false));
agentSend.addEventListener("click", sendAgentTurn);
agentCancel.addEventListener("click", cancelAgentTurn);
agentPrompt.addEventListener("keydown", (event) => {
	if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) {
		event.preventDefault();
		sendAgentTurn();
	}
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
aggregateMenu.addEventListener("click", () => {
	if (!annotateVisible) setAnnotateVisible(true);
	doAggregateAnnotations();
});
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
previewEl.addEventListener("mouseup", () => setTimeout(showPreviewSelectionAction));
previewEl.addEventListener("click", (event) => {
	const target =
		event.target instanceof Element ? event.target : event.target?.parentElement;
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
selectionAnnotateButton.addEventListener("click", () => {
	const range = pendingPreviewSelection;
	hideSelectionAnnotate();
	if (range) beginAnnotation(range);
});
document.addEventListener("pointerdown", (event) => {
	if (
		!selectionAnnotateButton.hidden &&
		!selectionAnnotateButton.contains(event.target) &&
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
	if (event.key === "Escape" && !fileMenu.hidden) {
		event.preventDefault();
		setFileMenuOpen(false);
		fileMenuButton.focus();
		return;
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
		saveCurrent();
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
		setSidebarVisible(!sidebarVisible);
	}
	if (key === "m" && event.shiftKey) {
		event.preventDefault();
		beginAnnotation();
	}
	if (key === "1") {
		event.preventDefault();
		setViewMode("editor");
	}
	if (key === "2") {
		event.preventDefault();
		setViewMode("split");
	}
	if (key === "3") {
		event.preventDefault();
		setViewMode("reader");
	}
});

applyLayout();
restoreWorkspace();
