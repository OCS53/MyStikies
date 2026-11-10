import { getCurrentWindow } from "@tauri-apps/api/window";
import "./styles.css";
import { mountNote } from "./note.js";
import { mountFinder } from "./finder.js";
import { mountSettings } from "./settings.js";

const root = document.querySelector("#app");
const label = getCurrentWindow().label;

if (label === "finder") {
  mountFinder(root);
} else if (label === "settings") {
  mountSettings(root);
} else if (label.startsWith("n-")) {
  mountNote(root, label.slice(2));
} else {
  root.innerHTML = `<div class="panel"><p class="muted">이 창을 열 수 없습니다.</p></div>`;
}
