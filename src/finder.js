import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { formatWhen } from "./colors.js";

export function mountFinder(root) {
  root.innerHTML = `
    <section class="panel finder">
      <header class="top">
        <div data-tauri-drag-region>
          <p class="eyebrow" data-tauri-drag-region>Stikies</p>
          <h1 data-tauri-drag-region>노트 목록</h1>
        </div>
        <button type="button" class="icon close" id="close" title="닫기">×</button>
      </header>
      <div class="search-row">
        <input id="query" type="search" placeholder="제목, 내용 검색" autocomplete="off" spellcheck="false" />
        <button type="button" id="create" class="primary">새 노트</button>
      </div>
      <div class="segments" role="tablist">
        <button type="button" class="on" data-archive="false">노트</button>
        <button type="button" data-archive="true">휴지통</button>
      </div>
      <div id="list" class="list"></div>
      <footer class="panel-foot">
        <span>↑↓ 이동 · Enter 열기</span>
        <button type="button" id="empty" class="text danger" hidden>휴지통 비우기</button>
      </footer>
    </section>
  `;

  const query = root.querySelector("#query");
  const list = root.querySelector("#list");
  const empty = root.querySelector("#empty");
  let archived = false;
  let notes = [];
  let active = 0;

  const filtered = () => {
    const needle = query.value.trim().toLowerCase();
    if (!needle) return notes;
    return notes.filter((note) =>
      `${note.title} ${note.snippet}`.toLowerCase().includes(needle),
    );
  };

  const paint = () => {
    const items = filtered();
    if (active >= items.length) active = Math.max(0, items.length - 1);
    empty.hidden = !archived || notes.length === 0;
    const needle = query.value.trim();
    root.querySelector(".eyebrow").textContent = archived
      ? `휴지통 ${notes.length}개`
      : `노트 ${notes.length}개`;
    if (!items.length) {
      const message = archived
        ? "휴지통이 비어 있습니다."
        : needle
          ? "찾는 노트가 없습니다."
          : "노트가 없습니다. 새 노트로 시작하세요.";
      list.innerHTML = `<p class="empty">${message}</p>`;
      return;
    }
    list.innerHTML = items
      .map((note, index) => {
        const when = note.remindAtMs ? `<em>${escapeHtml(formatWhen(note.remindAtMs))}</em>` : "";
        return `
          <button type="button" class="row ${index === active ? "active" : ""}" data-id="${note.id}">
            <i data-color="${note.color}"></i>
            <span>
              <strong>${escapeHtml(note.title || "빈 노트")}</strong>
              <small>${escapeHtml(note.snippet || "내용 없음")}${note.isOpen ? " · 화면에 있음" : ""}</small>
            </span>
            ${when}
          </button>`;
      })
      .join("");
  };

  const load = async () => {
    notes = await invoke("list_notes", { archived });
    paint();
  };

  root.querySelector("#close").addEventListener("click", () => {
    void getCurrentWindow().close();
  });
  root.querySelector("#create").addEventListener("click", () => {
    void invoke("create_note");
  });
  root.querySelectorAll(".segments button").forEach((button) => {
    button.addEventListener("click", () => {
      archived = button.dataset.archive === "true";
      root.querySelectorAll(".segments button").forEach((item) => item.classList.toggle("on", item === button));
      active = 0;
      void load();
    });
  });
  query.addEventListener("input", () => {
    active = 0;
    paint();
  });
  query.addEventListener("keydown", (event) => {
    const items = filtered();
    if (event.key === "ArrowDown") {
      active = Math.min(active + 1, Math.max(0, items.length - 1));
      paint();
      event.preventDefault();
    } else if (event.key === "ArrowUp") {
      active = Math.max(active - 1, 0);
      paint();
      event.preventDefault();
    } else if (event.key === "Enter" && items[active]) {
      void openNote(items[active]);
    }
  });
  list.addEventListener("click", (event) => {
    const row = event.target.closest(".row");
    if (!row) return;
    const note = notes.find((item) => item.id === row.dataset.id);
    if (note) void openNote(note);
  });
  empty.addEventListener("click", async () => {
    if (!confirm("휴지통의 노트와 이미지를 완전히 지울까요?")) return;
    await invoke("empty_trash");
    await load();
  });

  async function openNote(note) {
    if (archived) await invoke("restore_note", { id: note.id });
    else await invoke("focus_note", { id: note.id });
    await load();
  }

  void listen("library-changed", () => void load());
  void load().then(() => query.focus());
}

function escapeHtml(value) {
  return String(value).replace(/[&<>"]/g, (char) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[char]);
}
