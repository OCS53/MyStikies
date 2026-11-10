import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { COLORS, formatWhen } from "./colors.js";

const SCALES = [0.92, 1, 1.14, 1.28];

export async function mountNote(root, id) {
  root.innerHTML = noteTemplate();
  const paper = root.querySelector(".paper");
  const editor = root.querySelector(".editor");
  const pin = root.querySelector('[data-act="pin"]');
  const status = root.querySelector(".status");
  const reminder = root.querySelector(".reminder");
  let note = null;
  let ready = false;
  let saveTimer = 0;
  let saving = false;

  const toast = (message) => {
    const el = root.querySelector(".toast");
    el.textContent = message;
    el.classList.add("show");
    clearTimeout(toast._t);
    toast._t = setTimeout(() => el.classList.remove("show"), 2600);
  };

  const apply = (next) => {
    note = next;
    paper.dataset.color = next.color;
    paper.classList.toggle("collapsed", next.collapsed);
    paper.style.setProperty("--scale", String(next.fontScale || 1));
    const alpha = clampOpacity(next.opacity);
    paper.style.setProperty("--note-alpha", String(alpha));
    const slider = root.querySelector("[data-opacity]");
    const opacityLabel = root.querySelector("[data-opacity-label]");
    if (slider && document.activeElement !== slider) slider.value = String(Math.round(alpha * 100));
    if (opacityLabel) opacityLabel.textContent = `${Math.round(alpha * 100)}%`;
    pin.classList.toggle("on", next.pinned);
    pin.title = next.pinned ? "항상 위 해제" : "항상 위";
    editor.classList.toggle("is-empty", textOf(editor).trim().length === 0);
    if (next.remindAtMs) {
      reminder.hidden = false;
      reminder.textContent = `${formatWhen(next.remindAtMs)}에 다시 열기`;
    } else {
      reminder.hidden = true;
    }
    status.textContent = next.updatedAtMs ? formatSaved(next.updatedAtMs) : "새 노트";
  };

  const loadAssets = async () => {
    const assets = await invoke("note_assets", { noteId: id });
    const map = new Map(assets.map((asset) => [asset.id, asset]));
    editor.querySelectorAll("img").forEach((img) => {
      const asset = map.get(img.dataset.assetId || "");
      if (!asset) {
        if (!img.src.startsWith("data:") && !img.src.startsWith("blob:")) img.remove();
        return;
      }
      img.src = `data:${asset.mime};base64,${asset.dataBase64}`;
    });
    syncTasks(editor);
  };

  const reload = async () => {
    const next = await invoke("get_note", { id });
    const hadFocus = document.activeElement === editor;
    const html = next.bodyHtml || "";
    if (editor.innerHTML !== html && document.activeElement !== editor) {
      editor.innerHTML = html;
      await loadAssets();
    } else if (!ready) {
      editor.innerHTML = html;
      await loadAssets();
    }
    apply(next);
    if (hadFocus) editor.focus();
  };

  try {
    await reload();
    ready = true;
  } catch (error) {
    paper.innerHTML = `<p class="load-error">노트를 열지 못했습니다.<br>${escapeHtml(String(error))}</p>`;
    return;
  }

  const flushSave = async () => {
    if (!ready || saving) return;
    clearTimeout(saveTimer);
    const bodyHtml = serialise(editor);
    const title = titleFrom(editor);
    if (note && note.bodyHtml === bodyHtml && note.title === title) return;
    saving = true;
    try {
      await invoke("save_note", { id, title, bodyHtml });
      note = { ...note, title, bodyHtml, updatedAtMs: Date.now() };
      status.textContent = "방금 저장";
    } catch (error) {
      toast(String(error));
    } finally {
      saving = false;
    }
  };

  const scheduleSave = () => {
    editor.classList.toggle("is-empty", textOf(editor).trim().length === 0);
    clearTimeout(saveTimer);
    saveTimer = setTimeout(() => void flushSave(), 220);
  };

  editor.addEventListener("input", () => {
    if (ready) scheduleSave();
  });
  editor.addEventListener("focusout", () => void flushSave());

  editor.addEventListener("paste", (event) => {
    void onPaste(event, editor, id, toast, scheduleSave);
  });
  editor.addEventListener("dragover", (event) => {
    if ([...event.dataTransfer?.items || []].some((item) => item.kind === "file")) {
      event.preventDefault();
    }
  });
  editor.addEventListener("drop", (event) => {
    const files = [...event.dataTransfer?.files || []].filter((file) => file.type.startsWith("image/"));
    if (!files.length) return;
    event.preventDefault();
    void (async () => {
      for (const file of files) await insertImageFile(editor, id, file, toast);
      scheduleSave();
    })();
  });
  editor.addEventListener("mousedown", (event) => {
    const box = event.target.closest?.(".box");
    if (!box || !editor.contains(box)) return;
    event.preventDefault();
    const checked = box.dataset.checked !== "true";
    box.dataset.checked = checked ? "true" : "false";
    box.closest(".task")?.classList.toggle("done", checked);
    scheduleSave();
  });
  editor.addEventListener("click", (event) => {
    const link = event.target.closest?.("a");
    if (!link || !editor.contains(link)) return;
    event.preventDefault();
    const url = link.getAttribute("href");
    if (url) invoke("open_url", { url }).catch((error) => toast(String(error)));
  });

  root.querySelectorAll(".edge").forEach((edge) => {
    edge.addEventListener("mousedown", (event) => {
      if (paper.classList.contains("collapsed") || event.button !== 0) return;
      event.stopPropagation();
      void getCurrentWindow().startResizeDragging(edge.dataset.dir);
    });
  });
  root.querySelector(".top").addEventListener("dblclick", (event) => {
    if (event.target.closest("button, input, .bar-opacity")) return;
    const collapsed = !note.collapsed;
    invoke("set_note_collapsed", { id, collapsed })
      .then(() => apply({ ...note, collapsed }))
      .catch((error) => toast(String(error)));
  });

  const closePopovers = () => {
    root.querySelectorAll(".popover").forEach((el) => {
      el.hidden = true;
    });
  };
  const togglePopover = (name, anchor) => {
    const pop = root.querySelector(`[data-pop="${name}"]`);
    const willOpen = pop.hidden;
    closePopovers();
    if (!willOpen) return;
    placePopover(pop, anchor);
  };

  root.addEventListener("click", (event) => {
    const action = event.target.closest?.("[data-act]")?.dataset.act;
    if (!action) {
      if (!event.target.closest(".popover")) closePopovers();
      return;
    }
    if (action === "color" || action === "more" || action === "remind") {
      event.stopPropagation();
      if (action === "remind") fillRemindFields();
      togglePopover(action, event.target.closest("[data-act]") || event.target);
      return;
    }
    closePopovers();
    void runAction(action);
  });

  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape") closePopovers();
  });

  async function runAction(action) {
    try {
      if (action === "pin") {
        const pinned = !note.pinned;
        await invoke("set_note_pinned", { id, pinned });
        apply({ ...note, pinned });
      } else if (action === "close") {
        await flushSave();
        await invoke("dismiss_note", { id });
      } else if (action === "task") {
        insertTask(editor);
        scheduleSave();
      } else if (action === "paste") {
        await pasteFromSystem(editor, id, toast);
        scheduleSave();
      } else if (action === "duplicate") {
        await flushSave();
        await invoke("duplicate_note", { id });
      } else if (action === "list") {
        await invoke("open_note_list");
      } else if (action === "font") {
        const current = note.fontScale || 1;
        const next = SCALES[(SCALES.findIndex((scale) => Math.abs(scale - current) < 0.02) + 1) % SCALES.length] || 1;
        await invoke("set_note_font_scale", { id, fontScale: next });
        apply({ ...note, fontScale: next });
      } else if (action === "remind-save") {
        const hour = Number(root.querySelector("[data-remind-hour]").value);
        const minute = Number(root.querySelector("[data-remind-minute]").value);
        await setReminder(nextClock(hour, minute));
      } else if (action === "remind-clear") {
        await setReminder(null);
      } else if (action === "archive") {
        await flushSave();
        await invoke("archive_note", { id });
      } else if (action.startsWith("color-")) {
        const color = action.slice("color-".length);
        await invoke("set_note_color", { id, color });
        apply({ ...note, color });
      }
    } catch (error) {
      toast(String(error));
    }
  }

  function fillRemindFields() {
    const when = note.remindAtMs ? new Date(note.remindAtMs) : new Date(Date.now() + 60 * 1000);
    root.querySelector("[data-remind-hour]").value = String(when.getHours());
    root.querySelector("[data-remind-minute]").value = String(when.getMinutes());
  }

  async function setReminder(remindAtMs) {
    await invoke("set_reminder", { id, remindAtMs });
    apply({ ...note, remindAtMs });
  }

  const slider = root.querySelector("[data-opacity]");
  let opacityTimer = 0;
  slider.addEventListener("pointerdown", (event) => event.stopPropagation());
  slider.addEventListener("input", () => {
    const opacity = clampOpacity(Number(slider.value) / 100);
    apply({ ...note, opacity });
    clearTimeout(opacityTimer);
    opacityTimer = setTimeout(() => {
      invoke("set_note_opacity", { id, opacity }).catch((error) => toast(String(error)));
    }, 120);
  });

  await listen("note-meta", () => {
    invoke("get_note", { id }).then(apply).catch(() => {});
  });
}

function noteTemplate() {
  const swatches = COLORS.map(
    (color) =>
      `<button type="button" class="swatch" data-act="color-${color.id}" data-color="${color.id}" title="${color.name}"></button>`,
  ).join("");
  const hours = Array.from({ length: 24 }, (_, hour) => `<option value="${hour}">${String(hour).padStart(2, "0")}</option>`).join("");
  const minutes = Array.from({ length: 60 }, (_, minute) => `<option value="${minute}">${String(minute).padStart(2, "0")}</option>`).join("");
  return `
    <article class="paper note" data-color="sun">
      <header class="top">
        <div class="drag-fill" data-tauri-drag-region></div>
        <div class="grip" data-tauri-drag-region title="끌어서 이동 · 두 번 누르면 접기"><span></span><span></span><span></span></div>
        <label class="bar-opacity" title="투명도">
          <input data-opacity type="range" min="40" max="100" value="100" />
          <b data-opacity-label>100%</b>
        </label>
        <div class="actions">
          <button type="button" class="icon" data-act="list" title="노트 목록">${svg.list}</button>
          <button type="button" class="icon" data-act="color" title="색">${svg.color}</button>
          <button type="button" class="icon" data-act="pin" title="항상 위">${svg.pin}</button>
          <button type="button" class="icon" data-act="more" title="더보기">${svg.more}</button>
          <button type="button" class="icon close" data-act="close" title="노트 닫기">${svg.minus}</button>
        </div>
      </header>
      <div class="editor" contenteditable="true" spellcheck="true"></div>
      <footer>
        <span class="status">새 노트</span>
        <button type="button" class="reminder" data-act="remind" hidden></button>
      </footer>
      <div class="fold"></div>
      <div class="toast"></div>
      <div class="edge n" data-dir="North"></div>
      <div class="edge e" data-dir="East"></div>
      <div class="edge s" data-dir="South"></div>
      <div class="edge w" data-dir="West"></div>
      <div class="edge ne" data-dir="NorthEast"></div>
      <div class="edge nw" data-dir="NorthWest"></div>
      <div class="edge se" data-dir="SouthEast"></div>
      <div class="edge sw" data-dir="SouthWest"></div>
    </article>
    <div class="popover colors" data-pop="color" hidden>${swatches}</div>
    <div class="popover remind" data-pop="remind" hidden>
      <label class="clock">
        <select data-remind-hour aria-label="시">${hours}</select><span>시</span>
        <select data-remind-minute aria-label="분">${minutes}</select><span>분</span>
      </label>
      <div class="remind-actions">
        <button type="button" data-act="remind-save">설정</button>
        <button type="button" data-act="remind-clear">끄기</button>
      </div>
    </div>
    <div class="popover menu" data-pop="more" hidden>
      <button type="button" data-act="task">체크리스트</button>
      <button type="button" data-act="paste">붙여넣기</button>
      <button type="button" data-act="duplicate">복제</button>
      <button type="button" data-act="font">글자 크기</button>
      <hr>
      <button type="button" class="wide" data-act="remind">알림설정</button>
      <hr>
      <button type="button" class="danger" data-act="archive">휴지통으로</button>
    </div>
  `;
}

async function onPaste(event, editor, noteId, toast, scheduleSave) {
  const data = event.clipboardData;
  const items = [...data?.items || []];
  const files = items
    .filter((item) => item.kind === "file" && item.type.startsWith("image/"))
    .map((item) => item.getAsFile())
    .filter(Boolean);
  const html = data?.getData("text/html") || "";
  const text = data?.getData("text/plain") || "";
  if (!files.length && !html.trim() && !text && items.length === 0) {
    event.preventDefault();
    await pasteFromSystem(editor, noteId, toast);
    scheduleSave();
    return;
  }
  if (files.length && !html.trim() && !text.trim()) {
    event.preventDefault();
    for (const file of files) await insertImageFile(editor, noteId, file, toast);
    scheduleSave();
    return;
  }
  if (html.trim()) {
    event.preventDefault();
    insertNode(editor, sanitizeFragment(html));
    await adoptImages(editor, noteId, toast);
    scheduleSave();
    return;
  }
  if (text) {
    event.preventDefault();
    insertPlain(editor, text);
    scheduleSave();
    return;
  }
  if (files.length) {
    event.preventDefault();
    for (const file of files) await insertImageFile(editor, noteId, file, toast);
    scheduleSave();
  }
}

async function pasteFromSystem(editor, noteId, toast) {
  const clip = await invoke("read_clipboard");
  if (clip.kind === "image") {
    const saved = await invoke("save_asset", {
      noteId,
      mime: clip.mime || "image/png",
      dataBase64: clip.dataBase64,
    });
    insertImage(editor, saved.id, `data:${saved.mime};base64,${clip.dataBase64}`);
    return;
  }
  if (clip.kind === "text" && clip.text) {
    insertPlain(editor, clip.text);
    return;
  }
  toast("클립보드가 비어 있습니다");
}

async function insertImageFile(editor, noteId, file, toast) {
  try {
    const dataBase64 = await blobToBase64(file);
    const saved = await invoke("save_asset", {
      noteId,
      mime: file.type || "image/png",
      dataBase64,
    });
    insertImage(editor, saved.id, `data:${saved.mime};base64,${dataBase64}`);
  } catch (error) {
    toast(String(error));
  }
}

async function adoptImages(editor, noteId, toast) {
  const images = [...editor.querySelectorAll("img")].filter((img) => !img.dataset.assetId);
  for (const img of images) {
    const src = img.getAttribute("src") || "";
    if (!src.startsWith("data:image/") && !src.startsWith("blob:")) {
      img.replaceWith(document.createTextNode(img.alt || ""));
      continue;
    }
    try {
      const blob = src.startsWith("blob:") ? await (await fetch(src)).blob() : dataUrlToBlob(src);
      const dataBase64 = await blobToBase64(blob);
      const saved = await invoke("save_asset", {
        noteId,
        mime: blob.type || "image/png",
        dataBase64,
      });
      img.dataset.assetId = saved.id;
      img.src = `data:${saved.mime};base64,${dataBase64}`;
    } catch (error) {
      toast(String(error));
      img.remove();
    }
  }
}

function insertImage(editor, id, src) {
  const img = document.createElement("img");
  img.dataset.assetId = id;
  img.src = src;
  img.alt = "";
  insertNode(editor, img);
}

function insertTask(editor) {
  const task = document.createElement("div");
  task.className = "task";
  task.innerHTML =
    '<button type="button" class="box" data-checked="false" contenteditable="false"></button><div class="task-text"><br></div>';
  insertNode(editor, task);
  const text = task.querySelector(".task-text");
  const range = document.createRange();
  range.selectNodeContents(text);
  range.collapse(true);
  const selection = getSelection();
  selection.removeAllRanges();
  selection.addRange(range);
}

function insertPlain(editor, text) {
  const lines = text.replace(/\r\n/g, "\n").split("\n");
  const frag = document.createDocumentFragment();
  lines.forEach((line, index) => {
    if (index) frag.appendChild(document.createElement("br"));
    frag.appendChild(document.createTextNode(line));
  });
  insertNode(editor, frag);
}

function insertNode(editor, node) {
  editor.focus();
  const selection = getSelection();
  if (!selection || !selection.rangeCount || !editor.contains(selection.anchorNode)) {
    editor.appendChild(node);
    return;
  }
  const range = selection.getRangeAt(0);
  range.deleteContents();
  const last = node.nodeType === Node.DOCUMENT_FRAGMENT_NODE ? node.lastChild : node;
  range.insertNode(node);
  if (last && last.parentNode) {
    range.setStartAfter(last);
    range.collapse(true);
    selection.removeAllRanges();
    selection.addRange(range);
  }
}

function sanitizeFragment(html) {
  const template = document.createElement("template");
  template.innerHTML = html;
  return cleanChildren(template.content);
}

const ALLOWED = new Set([
  "B", "STRONG", "I", "EM", "U", "S", "STRIKE", "A", "P", "BR", "DIV", "SPAN",
  "UL", "OL", "LI", "IMG", "H1", "H2", "H3", "CODE", "PRE", "BLOCKQUOTE", "FIGURE", "BUTTON",
]);

function cleanChildren(node) {
  const frag = document.createDocumentFragment();
  for (const child of [...node.childNodes]) {
    if (child.nodeType === Node.TEXT_NODE) {
      frag.appendChild(document.createTextNode(child.textContent));
      continue;
    }
    if (child.nodeType !== Node.ELEMENT_NODE) continue;
    const tag = child.tagName;
    if (["SCRIPT", "STYLE", "IFRAME", "OBJECT", "EMBED", "META", "LINK", "FORM"].includes(tag)) continue;
    if (!ALLOWED.has(tag)) {
      frag.appendChild(cleanChildren(child));
      continue;
    }
    const el = document.createElement(tag.toLowerCase());
    if (tag === "A") {
      const href = child.getAttribute("href") || "";
      if (/^(https?:|mailto:)/i.test(href)) {
        el.setAttribute("href", href);
        el.setAttribute("rel", "noreferrer");
      }
    } else if (tag === "IMG") {
      if (child.dataset.assetId) el.dataset.assetId = child.dataset.assetId;
      const src = child.getAttribute("src") || "";
      if (src.startsWith("data:image/") || src.startsWith("blob:")) el.setAttribute("src", src);
      el.alt = "";
    } else if (tag === "BUTTON") {
      el.type = "button";
      el.className = "box";
      el.dataset.checked = child.dataset.checked === "true" ? "true" : "false";
      el.setAttribute("contenteditable", "false");
    } else if (tag === "DIV" && child.classList.contains("task")) {
      el.className = "task";
      if (child.classList.contains("done")) el.classList.add("done");
    } else if (tag === "DIV" && child.classList.contains("task-text")) {
      el.className = "task-text";
    }
    el.appendChild(cleanChildren(child));
    frag.appendChild(el);
  }
  return frag;
}

function serialise(editor) {
  const clone = editor.cloneNode(true);
  clone.querySelectorAll("img").forEach((img) => {
    if (!img.dataset.assetId) {
      img.remove();
      return;
    }
    img.removeAttribute("src");
  });
  return clone.innerHTML;
}

function titleFrom(editor) {
  const line = (editor.innerText || "")
    .split(/\r?\n/)
    .map((item) => item.trim())
    .find(Boolean);
  return (line || "").slice(0, 80);
}

function textOf(editor) {
  return editor.innerText || "";
}

function syncTasks(editor) {
  editor.querySelectorAll(".task").forEach((task) => {
    const box = task.querySelector(".box");
    task.classList.toggle("done", box?.dataset.checked === "true");
  });
}

function nextClock(hour, minute) {
  const when = new Date();
  when.setSeconds(0, 0);
  when.setHours(hour, minute, 0, 0);
  if (when.getTime() <= Date.now()) when.setDate(when.getDate() + 1);
  return when.getTime();
}

function placePopover(pop, anchor) {
  const margin = 8;
  pop.hidden = false;
  pop.style.left = "auto";
  pop.style.bottom = "auto";
  pop.style.maxHeight = "none";
  pop.style.top = "0px";
  pop.style.right = `${margin}px`;
  const rect = anchor.getBoundingClientRect();
  const vw = window.innerWidth;
  const vh = window.innerHeight;
  const width = Math.min(pop.offsetWidth, vw - margin * 2);
  let right = vw - rect.right;
  if (right + width > vw - margin) right = vw - margin - width;
  pop.style.right = `${Math.max(margin, right)}px`;
  const full = pop.scrollHeight;
  if (full + margin * 2 <= vh) {
    let top = rect.bottom + 6;
    if (top + full > vh - margin) top = vh - margin - full;
    pop.style.top = `${Math.max(margin, top)}px`;
    pop.style.maxHeight = `${full}px`;
    return;
  }
  pop.style.top = `${margin}px`;
  pop.style.maxHeight = `${Math.max(72, vh - margin * 2)}px`;
}

function clampOpacity(value) {
  const number = Number(value);
  if (!Number.isFinite(number)) return 1;
  return Math.min(1, Math.max(0.4, number));
}

function formatSaved(ms) {
  const delta = Date.now() - ms;
  if (delta < 20_000) return "방금 저장";
  return new Date(ms).toLocaleString("ko-KR", {
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  });
}

function blobToBase64(blob) {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result).split(",")[1] || "");
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(blob);
  });
}

function dataUrlToBlob(src) {
  const [meta, data] = src.split(",");
  const mime = /data:(.*?);/.exec(meta)?.[1] || "image/png";
  const bytes = Uint8Array.from(atob(data), (char) => char.charCodeAt(0));
  return new Blob([bytes], { type: mime });
}

function escapeHtml(value) {
  return value.replace(/[&<>"]/g, (char) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[char]);
}

const svg = {
  list: '<svg viewBox="0 0 24 24"><path d="M8 7h11M8 12h11M8 17h11" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"/><circle cx="4.6" cy="7" r="1.15" fill="currentColor"/><circle cx="4.6" cy="12" r="1.15" fill="currentColor"/><circle cx="4.6" cy="17" r="1.15" fill="currentColor"/></svg>',
  color: '<svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="5.4" fill="none" stroke="currentColor" stroke-width="1.8"/><circle cx="12" cy="12" r="2.1" fill="currentColor"/></svg>',
  pin: '<svg viewBox="0 0 24 24"><path d="M9 4.5h6l-.4 5.2 2.9 2.3H13V20l-1 1-1-1v-8H6.5l2.9-2.3L9 4.5Z" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linejoin="round"/></svg>',
  more: '<svg viewBox="0 0 24 24"><circle cx="6" cy="12" r="1.4" fill="currentColor"/><circle cx="12" cy="12" r="1.4" fill="currentColor"/><circle cx="18" cy="12" r="1.4" fill="currentColor"/></svg>',
  minus: '<svg viewBox="0 0 24 24"><path d="M6 12h12" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/></svg>',
};
