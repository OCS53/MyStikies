import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { COLORS, formatBytes } from "./colors.js";

export async function mountSettings(root) {
  root.innerHTML = `
    <section class="panel settings">
      <header class="top" data-tauri-drag-region>
        <div data-tauri-drag-region>
          <p class="eyebrow">Stikies</p>
          <h1 data-tauri-drag-region>설정</h1>
        </div>
        <button type="button" class="icon close" id="close" title="닫기">×</button>
      </header>
      <div class="settings-body">
        <section>
          <h2>데이터 폴더</h2>
          <p class="path" id="path"></p>
          <p class="muted" id="stats"></p>
          <p class="muted">노트는 notes.parquet, 붙여 넣은 이미지는 assets.parquet 로 저장됩니다. 설정은 AppData\\Stikies\\config.parquet 에 남습니다.</p>
          <div class="row-actions">
            <button type="button" id="pick" class="primary">폴더 변경</button>
            <button type="button" id="open-dir" class="ghost">폴더 열기</button>
          </div>
          <div class="choice" id="choice" hidden>
            <p id="choice-path"></p>
            <div class="row-actions">
              <button type="button" id="move" class="primary">현재 노트를 이 폴더로 옮기기</button>
              <button type="button" id="switch" class="ghost">이 폴더의 노트 열기</button>
              <button type="button" id="cancel" class="text">취소</button>
            </div>
          </div>
          <p class="warn" id="message" hidden></p>
        </section>
        <section>
          <h2>시작</h2>
          <label class="checkline"><input type="checkbox" id="login"> 윈도우 시작 시 자동 실행</label>
          <label class="checkline"><input type="checkbox" id="restore"> 실행할 때 열어 둔 노트 복원</label>
        </section>
        <section>
          <h2>새 노트 색</h2>
          <div class="swatches" id="colors"></div>
        </section>
        <section>
          <h2>단축키</h2>
          <ul class="keys">
            <li><span>새 노트</span><kbd>Ctrl</kbd><kbd>Shift</kbd><kbd>N</kbd></li>
            <li><span>노트 목록</span><kbd>Ctrl</kbd><kbd>Shift</kbd><kbd>F</kbd></li>
            <li><span>모두 숨기기 / 보이기</span><kbd>Ctrl</kbd><kbd>Shift</kbd><kbd>H</kbd></li>
          </ul>
          <p class="muted">트레이 아이콘을 누르면 메뉴가 열립니다. 노트를 닫아도 Stikies는 종료되지 않습니다.</p>
        </section>
      </div>
    </section>
  `;

  const message = root.querySelector("#message");
  const choice = root.querySelector("#choice");
  let pendingPath = "";
  let settings = null;

  const showMessage = (text) => {
    message.hidden = !text;
    message.textContent = text || "";
  };

  const paint = () => {
    root.querySelector("#path").textContent = settings.dataDir;
    root.querySelector("#stats").textContent =
      `노트 ${settings.noteCount}개 · notes.parquet ${formatBytes(settings.notesBytes)} · assets.parquet ${formatBytes(settings.assetsBytes)}`;
    root.querySelector("#login").checked = settings.openOnLogin;
    root.querySelector("#restore").checked = settings.restoreOnStart;
    root.querySelector("#colors").innerHTML = COLORS.map(
      (color) =>
        `<button type="button" class="swatch ${settings.defaultColor === color.id ? "on" : ""}" data-color="${color.id}" title="${color.name}"></button>`,
    ).join("");
  };

  const load = async () => {
    settings = await invoke("get_settings");
    paint();
  };

  root.querySelector("#close").addEventListener("click", () => void getCurrentWindow().close());
  root.querySelector("#open-dir").addEventListener("click", () => {
    invoke("open_data_dir").catch((error) => showMessage(String(error)));
  });
  root.querySelector("#pick").addEventListener("click", async () => {
    showMessage("");
    const picked = await invoke("pick_data_dir");
    if (!picked) return;
    pendingPath = picked;
    root.querySelector("#choice-path").textContent = picked;
    choice.hidden = false;
  });
  root.querySelector("#cancel").addEventListener("click", () => {
    choice.hidden = true;
    pendingPath = "";
  });
  root.querySelector("#move").addEventListener("click", () => void change("move"));
  root.querySelector("#switch").addEventListener("click", () => void change("switch"));
  root.querySelector("#login").addEventListener("change", async (event) => {
    try {
      await invoke("set_open_on_login", { enabled: event.target.checked });
    } catch (error) {
      event.target.checked = !event.target.checked;
      showMessage(String(error));
    }
  });
  root.querySelector("#restore").addEventListener("change", async (event) => {
    await invoke("set_restore_on_start", { enabled: event.target.checked });
  });
  root.querySelector("#colors").addEventListener("click", async (event) => {
    const swatch = event.target.closest(".swatch");
    if (!swatch) return;
    await invoke("set_default_color", { color: swatch.dataset.color });
    settings.defaultColor = swatch.dataset.color;
    paint();
  });

  async function change(mode) {
    try {
      settings = await invoke("change_data_dir", { path: pendingPath, mode });
      choice.hidden = true;
      pendingPath = "";
      paint();
      showMessage(mode === "move" ? "노트를 새 폴더로 옮겼습니다." : "선택한 폴더의 노트를 열었습니다.");
    } catch (error) {
      showMessage(String(error));
    }
  }

  await load();
}
