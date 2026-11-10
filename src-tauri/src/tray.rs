use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, Wry};

use crate::state::{lock_store, AppState};
use crate::windowing;

pub fn install(app: &tauri::App) -> tauri::Result<()> {
    let menu = build_menu(app.handle())?;
    let icon = app
        .default_window_icon()
        .cloned()
        .unwrap_or_else(|| tauri::include_image!("icons/32x32.png"));
    TrayIconBuilder::with_id("main")
        .tooltip("Stikies")
        .icon(icon)
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| handle(app, event.id.as_ref()))
        .build(app)?;
    refresh(app.handle());
    Ok(())
}

pub fn refresh(app: &AppHandle) {
    let Some(tray) = app.tray_by_id("main") else {
        return;
    };
    let count = {
        let state = app.state::<AppState>();
        let store = lock_store(&state);
        store.notes.iter().filter(|note| !note.archived).count()
    };
    let _ = tray.set_tooltip(Some(format!(
        "Stikies  ·  노트 {count}개  ·  Ctrl+Shift+N"
    )));
    if let Ok(menu) = build_menu(app) {
        let _ = tray.set_menu(Some(menu));
    }
}

fn handle(app: &AppHandle, id: &str) {
    match id {
        "new" => {
            windowing::create_note(app);
        }
        "find" => windowing::open_finder(app),
        "autostart" => toggle_startup(app),
        "show-all" => windowing::show_all(app),
        "hide-all" => windowing::hide_all(app),
        "settings" => windowing::open_settings(app),
        "quit" => crate::quit(app),
        other => {
            if let Some(note_id) = other.strip_prefix("open:") {
                windowing::show_note(app, note_id);
            }
        }
    }
}

fn toggle_startup(app: &AppHandle) {
    let enabled = {
        let state = app.state::<AppState>();
        let store = lock_store(&state);
        !store.settings.open_on_login
    };
    if let Err(err) = crate::set_startup(app, enabled) {
        eprintln!("시작 프로그램 설정을 바꾸지 못했습니다: {err}");
    }
    refresh(app);
}

fn build_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let startup = {
        let state = app.state::<AppState>();
        let store = lock_store(&state);
        store.settings.open_on_login
    };
    let new_item = MenuItem::with_id(app, "new", "새 노트    Ctrl+Shift+N", true, None::<&str>)?;
    let find_item = MenuItem::with_id(app, "find", "노트 목록    Ctrl+Shift+F", true, None::<&str>)?;
    let startup_item = CheckMenuItem::with_id(
        app,
        "autostart",
        "윈도우 시작 시 실행",
        true,
        startup,
        None::<&str>,
    )?;
    let show_item = MenuItem::with_id(app, "show-all", "모든 노트 보이기", true, None::<&str>)?;
    let hide_item = MenuItem::with_id(
        app,
        "hide-all",
        "모든 노트 숨기기    Ctrl+Shift+H",
        true,
        None::<&str>,
    )?;
    let settings_item = MenuItem::with_id(app, "settings", "설정", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "종료", true, None::<&str>)?;
    let sep_top = PredefinedMenuItem::separator(app)?;
    let sep_mid = PredefinedMenuItem::separator(app)?;
    let sep_bot = PredefinedMenuItem::separator(app)?;
    let recent = MenuItem::with_id(app, "recent", "최근 노트", false, None::<&str>)?;

    let notes = {
        let state = app.state::<AppState>();
        let store = lock_store(&state);
        let mut notes: Vec<_> = store
            .notes
            .iter()
            .filter(|note| !note.archived)
            .cloned()
            .collect();
        notes.sort_by(|a, b| b.updated_at_ms.cmp(&a.updated_at_ms));
        notes.truncate(12);
        notes
    };

    let mut note_items = Vec::new();
    for note in &notes {
        let raw = if note.title.trim().is_empty() {
            "빈 노트".to_string()
        } else {
            note.title.replace(['\n', '\r'], " ")
        };
        let raw: String = raw.chars().take(28).collect();
        let mark = if note.is_open { "●" } else { "○" };
        note_items.push(MenuItem::with_id(
            app,
            format!("open:{}", note.id),
            format!("{mark}  {raw}"),
            true,
            None::<&str>,
        )?);
    }

    let mut refs: Vec<&dyn tauri::menu::IsMenuItem<Wry>> = vec![
        &new_item,
        &find_item,
        &startup_item,
        &show_item,
        &hide_item,
        &sep_top,
    ];
    if !note_items.is_empty() {
        refs.push(&recent);
        for item in &note_items {
            refs.push(item);
        }
        refs.push(&sep_mid);
    }
    refs.push(&settings_item);
    refs.push(&sep_bot);
    refs.push(&quit_item);
    Menu::with_items(app, &refs)
}
