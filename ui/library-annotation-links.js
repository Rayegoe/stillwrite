(function (root, factory) {
	const api = factory();
	if (typeof module === "object" && module.exports) module.exports = api;
	root.StillwriteLibraryAnnotationLinks = api;
	if (typeof document !== "undefined") api.install();
})(typeof globalThis !== "undefined" ? globalThis : window, function () {
	"use strict";

	const LINK_PREFIX = "<!-- stillwrite-linked-library:";
	const LINK_SUFFIX = " -->";

	function encodeMeta(value) {
		const json = JSON.stringify(value);
		if (typeof Buffer !== "undefined")
			return Buffer.from(json, "utf8").toString("base64url");
		const bytes = new TextEncoder().encode(json);
		let binary = "";
		bytes.forEach((byte) => {
			binary += String.fromCharCode(byte);
		});
		return btoa(binary)
			.replaceAll("+", "-")
			.replaceAll("/", "_")
			.replace(/=+$/g, "");
	}

	function decodeMeta(value) {
		try {
			if (typeof Buffer !== "undefined")
				return JSON.parse(Buffer.from(value, "base64url").toString("utf8"));
			const padded = value.replaceAll("-", "+").replaceAll("_", "/");
			const binary = atob(padded.padEnd(Math.ceil(padded.length / 4) * 4, "="));
			const bytes = Uint8Array.from(binary, (char) => char.charCodeAt(0));
			return JSON.parse(new TextDecoder().decode(bytes));
		} catch (_) {
			return null;
		}
	}

	function linkedMarker(meta) {
		return `${LINK_PREFIX}${encodeMeta(meta)}${LINK_SUFFIX}`;
	}

	function parseLinkedNote(note) {
		const text = String(note || "");
		if (!text.startsWith(LINK_PREFIX)) return null;
		const firstLineEnd = text.indexOf("\n");
		const firstLine = firstLineEnd >= 0 ? text.slice(0, firstLineEnd) : text;
		if (!firstLine.endsWith(LINK_SUFFIX)) return null;
		const encoded = firstLine.slice(LINK_PREFIX.length, -LINK_SUFFIX.length);
		const meta = decodeMeta(encoded);
		if (!meta?.uri || !meta?.sourceId || !meta?.relativePath) return null;
		return {
			meta,
			note: firstLineEnd >= 0 ? text.slice(firstLineEnd + 1).trimStart() : "",
		};
	}

	function linkedItemId(source, originId) {
		return `linked-${encodeMeta(`${source.uri}\u001f${originId}`).slice(0, 72)}`;
	}

	function makeLinkedItem(source, item) {
		const meta = {
			uri: source.uri,
			sourceId: source.sourceId,
			relativePath: source.relativePath,
			title: source.title || source.relativePath,
			sourceName: source.sourceName || "资料",
			originId: String(item.id || ""),
		};
		return {
			id: linkedItemId(source, meta.originId),
			kind: "selection",
			start: -1,
			end: -1,
			quote: String(item.quote || "整篇资料").trim(),
			note: `${linkedMarker(meta)}\n${String(item.note || "").trim()}`.trimEnd(),
			createdAt: String(item.createdAt || ""),
			updatedAt: String(item.updatedAt || item.createdAt || ""),
		};
	}

	function syncLinkedItems(existingItems, source, libraryItems) {
		const kept = (existingItems || []).filter((item) => {
			const linked = parseLinkedNote(item.note);
			return !linked || linked.meta.uri !== source.uri;
		});
		const linked = (libraryItems || [])
			.filter((item) => String(item.note || "").trim() || String(item.quote || "").trim())
			.map((item) => makeLinkedItem(source, item));
		return [...kept, ...linked];
	}

	function install() {
		if (typeof openRelatedItem !== "function" || typeof saveAnnotate !== "function") return;
		const codec = globalThis.StillwriteAnnotations;
		if (!codec) return;

		let writingAnchor = null;
		let openingForWritingAnchor = false;

		function currentWritingAnchor() {
			if (!currentFile || currentLibraryDocument || currentAgentDocument) return null;
			return {
				path: currentFile,
				title:
					documentTitleEl?.textContent ||
					basename(currentFile).replace(/\.(md|markdown)$/i, ""),
			};
		}

		const originalOpenLibraryDocument = openLibraryDocument;
		openLibraryDocument = async function (hit) {
			if (!openingForWritingAnchor) writingAnchor = null;
			return originalOpenLibraryDocument(hit);
		};

		const originalOpenRelatedItem = openRelatedItem;
		openRelatedItem = async function (item) {
			if (item?.kind !== "library") return originalOpenRelatedItem(item);
			const anchor = currentWritingAnchor();
			if (anchor) writingAnchor = anchor;
			openingForWritingAnchor = Boolean(anchor);
			try {
				return await originalOpenRelatedItem(item);
			} finally {
				openingForWritingAnchor = false;
			}
		};

		async function openLinkedLibrary(meta) {
			const anchor = currentWritingAnchor();
			if (anchor) writingAnchor = anchor;
			openingForWritingAnchor = Boolean(anchor);
			try {
				await openLibraryDocument({
					uri: meta.uri,
					source_id: meta.sourceId,
					relative_path: meta.relativePath,
					title: meta.title,
				});
			} finally {
				openingForWritingAnchor = false;
			}
		}

		async function syncLibraryAnnotationsToWritingAnchor() {
			if (!writingAnchor?.path || !currentLibraryDocument) return false;
			const source = {
				uri: currentLibraryDocument.uri,
				sourceId: currentLibraryDocument.source_id,
				relativePath: currentLibraryDocument.relative_path,
				title: currentLibraryDocument.title,
				sourceName: currentLibraryDocument.source_name,
			};
			try {
				const target = { kind: "workspace", path: writingAnchor.path };
				const data = await invoke("read_annotation", { target });
				const existing = codec.parse(data.body || "", data.updated_at || "");
				const merged = syncLinkedItems(existing, source, annotationItems);
				await invoke("save_annotation", {
					target,
					body: codec.serialize(merged),
				});
				if (autoSync) scheduleAutoSync();
				annotateFoot.textContent = `资料批注已保存，并关联到「${writingAnchor.title}」`;
				return true;
			} catch (error) {
				console.warn("资料批注关联到写作作品失败", error);
				annotateFoot.textContent = "资料批注已保存；关联到当前作品失败";
				return false;
			}
		}

		const originalSaveAnnotate = saveAnnotate;
		saveAnnotate = async function () {
			const target = currentDocumentRef();
			const librarySave = target?.kind === "library";
			await originalSaveAnnotate();
			if (librarySave && !annotateDirty) await syncLibraryAnnotationsToWritingAnchor();
		};

		const originalRenderAnnotationPanel = renderAnnotationPanel;
		renderAnnotationPanel = function () {
			originalRenderAnnotationPanel();
			for (const item of annotationItems) {
				const linked = parseLinkedNote(item.note);
				if (!linked) continue;
				const card = annotationList.querySelector(
					`.annotation-card[data-annotation-id="${CSS.escape(item.id)}"]`,
				);
				if (!card) continue;
				card.classList.add("linked-library-annotation");
				const kind = card.querySelector(".annotation-kind");
				if (kind) kind.textContent = "资料批注";
				const meta = card.querySelector(".annotation-card-meta");
				if (meta && !card.querySelector(".linked-library-source")) {
					const source = document.createElement("button");
					source.type = "button";
					source.className = "text-btn linked-library-source";
					source.textContent = `↗ ${linked.meta.title || linked.meta.relativePath}`;
					source.title = linked.meta.uri;
					source.addEventListener("click", () => void openLinkedLibrary(linked.meta));
					meta.after(source);
				}
				const quote = card.querySelector(".annotation-card-quote");
				if (quote) {
					quote.title = "打开资料原文";
					quote.addEventListener(
						"click",
						(event) => {
							event.preventDefault();
							event.stopImmediatePropagation();
							void openLinkedLibrary(linked.meta);
						},
						true,
					);
				}
				const note = card.querySelector(".annotation-card-note");
				if (note) {
					note.value = linked.note;
					note.readOnly = true;
					note.title = "资料批注请在原资料中修改";
				}
				const remove = card.querySelector(".annotation-delete");
				if (remove) remove.hidden = true;
			}
		};

		const originalRenderAnnotationAnchors = renderAnnotationAnchors;
		renderAnnotationAnchors = function () {
			const all = annotationItems;
			annotationItems = all.filter((item) => !parseLinkedNote(item.note));
			try {
				originalRenderAnnotationAnchors();
			} finally {
				annotationItems = all;
			}
		};

		const originalRenderMarkdown = renderMarkdown;
		renderMarkdown = function (source) {
			const cleaned = String(source || "")
				.split("\n")
				.filter((line) => !line.trim().startsWith(LINK_PREFIX))
				.join("\n");
			return originalRenderMarkdown(cleaned);
		};
	}

	return {
		LINK_PREFIX,
		parseLinkedNote,
		makeLinkedItem,
		syncLinkedItems,
		install,
	};
});
