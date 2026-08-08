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

const DEFAULT_REMOTE = "radxa@192.168.100.106:~/stillwrite.git";
let autoSync = false; // 首次手动同步成功后开启自动同步
let syncTimer = null;
const searchTimer = null;
let lastTreeNodes = [];

function escapeHtml(value) {
	return value
		.replaceAll("&", "&amp;")
		.replaceAll("<", "&lt;")
		.replaceAll(">", "&gt;")
		.replaceAll('"', "&quot;")
		.replaceAll("'", "&#039;");
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
	value = value.replace(/\[([^\]]+)\]\(([^)\s]+)\)/g, (_, label, href) => {
		const raw = href.replaceAll("&amp;", "&");
		const safe = /^(https?:|mailto:|#|\.\.?\/)/i.test(raw) ? href : "#";
		const target = /^https?:/i.test(raw)
			? ' target="_blank" rel="noreferrer"'
			: "";
		return `<a href="${safe}"${target}>${label}</a>`;
	});
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
		html.push(`<p>${renderInline(paragraph.join(" "))}</p>`);
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
	const allowed = new Set([
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
	[...doc.body.querySelectorAll("*")].reverse().forEach((el) => {
		if (!allowed.has(el.tagName)) {
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

function updatePreview() {
	const doc = sanitizeHtml(renderMarkdown(editor.value));
	previewEl.replaceChildren(...doc.body.childNodes);
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
	lastTreeNodes = data.nodes;
	renderTree(data.nodes);
}

async function chooseWorkspace() {
	try {
		// Save against the current workspace before Rust switches the active root.
		await saveCurrent();
		const data = await invoke("choose_workspace");
		if (!data) return;
		currentFile = null;
		editor.value = "";
		updatePreview();
		documentTitleEl.textContent = "Stillwrite";
		await useWorkspace(data);
	} catch (error) {
		console.error(error);
		markError("目录打开失败");
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
	try {
		const text = await invoke("read_markdown", { path });
		if (token !== loadToken) return;
		currentFile = path;
		editor.value = text;
		updatePreview();
		documentTitleEl.textContent = name.replace(/\.(md|markdown)$/i, "");
		document
			.querySelectorAll(".tree-file.active")
			.forEach((el) => el.classList.remove("active"));
		if (row) row.classList.add("active");
		editor.scrollTop = 0;
		previewEl.scrollTop = 0;
		markSaved();
	} catch (error) {
		console.error(error);
		markError("打开失败");
	}
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
	if (!nodes.length) {
		const tip = document.createElement("div");
		tip.className = "empty-tip";
		tip.textContent = "这个文件夹里还没有 Markdown 文件";
		treeEl.appendChild(tip);
		return;
	}
	const fragment = document.createDocumentFragment();
	nodes.forEach((node) => fragment.appendChild(treeNodeElement(node, 0)));
	treeEl.appendChild(fragment);
}

function treeNodeElement(node, depth) {
	if (node.is_dir) {
		const wrap = document.createElement("div");
		wrap.className = "tree-dir-wrap";
		const button = document.createElement("button");
		button.className = "tree-dir";
		button.style.paddingLeft = `${12 + depth * 14}px`;
		const chevron = document.createElement("span");
		chevron.className = "chevron";
		chevron.textContent = "⌄";
		const name = document.createElement("span");
		name.className = "node-name";
		name.textContent = node.name;
		button.append(chevron, name);
		const children = document.createElement("div");
		children.className = "tree-children";
		node.children.forEach((child) =>
			children.appendChild(treeNodeElement(child, depth + 1)),
		);
		button.addEventListener("click", () => {
			const collapsed = wrap.classList.toggle("collapsed");
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
	document.documentElement.style.setProperty(
		"--sidebar-width",
		`${sidebarWidth}px`,
	);
	editorPane.style.flexBasis = `${splitRatio}%`;
	readerPane.style.flexBasis = `${100 - splitRatio}%`;
	sidebar.classList.toggle("hidden", !sidebarVisible);
	sidebarHandle.classList.toggle("hidden", !sidebarVisible);
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

editor.addEventListener("input", () => {
	updatePreview();
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
document.querySelector("#refreshTree").addEventListener("click", refreshTree);
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
	const submitter = event.submitter;
	if (!submitter || submitter.value === "cancel") return;
	event.preventDefault();
	let requested = newFileName.value.trim();
	if (!requested) return;
	if (!/\.(md|markdown)$/i.test(requested)) requested += ".md";
	newFileDialog.close();
	createFile(requested);
});

window.addEventListener("keydown", (event) => {
	const mod = event.ctrlKey || event.metaKey;
	if (!mod) return;
	const key = event.key.toLowerCase();
	if (key === "o") {
		event.preventDefault();
		chooseWorkspace();
	}
	if (key === "s") {
		event.preventDefault();
		saveCurrent();
	}
	if (key === "n") {
		event.preventDefault();
		document.querySelector("#newFile").click();
	}
	if (key === "b") {
		event.preventDefault();
		setSidebarVisible(!sidebarVisible);
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
