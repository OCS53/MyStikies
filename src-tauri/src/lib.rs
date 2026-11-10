mod model;
mod state;
mod store;
mod tray;
mod windowing;

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use base64::Engine;
use image::codecs::png::PngEncoder;
use image::{ExtendedColorType, ImageEncoder};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

use crate::model::{
    clamp_opacity, color_or_default, now_ms, AssetDto, ClipDto, NoteDto, NoteSummary, SettingsDto,
};
use crate::state::{lock_store, AppState};
use crate::windowing::note_label;

static QUITTING: AtomicBool = AtomicBool::new(false);

pub fn run() {
    let state = AppState::load();
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            windowing::show_all(&app);
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .manage(state)
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            tray::install(app)?;
            register_shortcuts(app)?;
            let open_on_login = {
                let state = app.state::<AppState>();
                let store = lock_store(&state);
                store.settings.open_on_login
            };
            let _ = set_login(app.handle(), open_on_login);
            let restore = {
                let state = app.state::<AppState>();
                let store = lock_store(&state);
                store.settings.restore_on_start
            };
            if restore {
                windowing::show_open_notes(app.handle());
            }
            start_maintenance(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_note,
            list_notes,
            save_note,
            set_note_color,
            set_note_pinned,
            set_note_collapsed,
            set_note_font_scale,
            set_note_opacity,
            open_note_list,
            set_reminder,
            dismiss_note,
            archive_note,
            restore_note,
            destroy_note,
            empty_trash,
            duplicate_note,
            create_note,
            focus_note,
            save_asset,
            note_assets,
            read_clipboard,
            get_settings,
            pick_data_dir,
            change_data_dir,
            set_open_on_login,
            set_restore_on_start,
            set_default_color,
            open_data_dir,
            open_url,
        ])
        .build(tauri::generate_context!())
        .expect("Stikies를 시작하지 못했습니다")
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                if let Some(state) = app.try_state::<AppState>() {
                    lock_store(&state).flush();
                }
                if !QUITTING.load(Ordering::SeqCst) {
                    api.prevent_exit();
                }
            }
        });
}

pub fn quit(app: &AppHandle) {
    QUITTING.store(true, Ordering::SeqCst);
    if let Some(state) = app.try_state::<AppState>() {
        lock_store(&state).flush();
    }
    app.exit(0);
}

fn register_shortcuts(app: &tauri::App) -> tauri::Result<()> {
    let mods = Modifiers::CONTROL | Modifiers::SHIFT;
    let newer = Shortcut::new(Some(mods), Code::KeyN);
    let finder = Shortcut::new(Some(mods), Code::KeyF);
    let hider = Shortcut::new(Some(mods), Code::KeyH);
    app.handle().plugin(
        tauri_plugin_global_shortcut::Builder::new()
            .with_handler(move |app, shortcut, event| {
                if event.state() != ShortcutState::Pressed {
                    return;
                }
                if shortcut == &newer {
                    windowing::create_note(app);
                } else if shortcut == &finder {
                    windowing::open_finder(app);
                } else if shortcut == &hider {
                    windowing::toggle_notes(app);
                }
            })
            .build(),
    )?;
    let shortcuts = app.global_shortcut();
    for shortcut in [newer, finder, hider] {
        if let Err(err) = shortcuts.register(shortcut) {
            eprintln!("단축키를 등록하지 못했습니다: {err}");
        }
    }
    Ok(())
}

fn start_maintenance(app: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(800));
        if QUITTING.load(Ordering::SeqCst) {
            break;
        }
        {
            let state = app.state::<AppState>();
            lock_store(&state).flush();
        }
        fire_due_reminders(&app);
    });
}

fn fire_due_reminders(app: &AppHandle) {
    let now = now_ms();
    let due: Vec<String> = {
        let state = app.state::<AppState>();
        let mut store = lock_store(&state);
        let mut due = Vec::new();
        for note in &mut store.notes {
            if note.archived {
                continue;
            }
            if note.remind_at_ms.is_some_and(|at| at <= now) {
                note.remind_at_ms = None;
                note.is_open = true;
                note.collapsed = false;
                due.push(note.id.clone());
            }
        }
        if !due.is_empty() {
            store.mark_notes();
            store.flush();
        }
        due
    };
    if due.is_empty() {
        return;
    }
    for id in &due {
        let (width, height) = {
            let state = app.state::<AppState>();
            let store = lock_store(&state);
            store
                .note(id)
                .map(|note| (note.width, note.height))
                .unwrap_or((360.0, 460.0))
        };
        windowing::apply_collapsed(app, id, false, width, height);
        windowing::open_note(app, id);
        let _ = app.emit_to(note_label(id), "note-meta", ());
    }
    let _ = app.emit("library-changed", ());
    tray::refresh(app);
}

fn changed(app: &AppHandle) {
    let _ = app.emit("library-changed", ());
    tray::refresh(app);
}

#[tauri::command]
fn get_note(app: AppHandle, id: String) -> Result<NoteDto, String> {
    let state = app.state::<AppState>();
    let store = lock_store(&state);
    store.dto(&id).ok_or_else(|| "노트를 찾을 수 없습니다".into())
}

#[tauri::command]
fn list_notes(app: AppHandle, archived: bool) -> Vec<NoteSummary> {
    let state = app.state::<AppState>();
    let store = lock_store(&state);
    store.summaries(archived)
}

#[tauri::command]
fn save_note(app: AppHandle, id: String, title: String, body_html: String) -> Result<(), String> {
    {
        let state = app.state::<AppState>();
        let mut store = lock_store(&state);
        store.save_body(&id, title.clone(), body_html)?;
    }
    windowing::set_note_title(&app, &id, &title);
    changed(&app);
    Ok(())
}

#[tauri::command]
fn set_note_color(app: AppHandle, id: String, color: String) -> Result<(), String> {
    let color = color_or_default(&color);
    {
        let state = app.state::<AppState>();
        let mut store = lock_store(&state);
        let note = store.note_mut(&id).ok_or("노트를 찾을 수 없습니다")?;
        note.color = color;
        note.updated_at_ms = now_ms();
        store.mark_notes();
        store.flush();
    }
    changed(&app);
    Ok(())
}

#[tauri::command]
fn set_note_pinned(app: AppHandle, id: String, pinned: bool) -> Result<(), String> {
    {
        let state = app.state::<AppState>();
        let mut store = lock_store(&state);
        let note = store.note_mut(&id).ok_or("노트를 찾을 수 없습니다")?;
        note.pinned = pinned;
        store.mark_notes();
        store.flush();
    }
    windowing::apply_pinned(&app, &id, pinned);
    changed(&app);
    Ok(())
}

#[tauri::command]
fn set_note_collapsed(app: AppHandle, id: String, collapsed: bool) -> Result<(), String> {
    let (width, height) = {
        let state = app.state::<AppState>();
        let mut store = lock_store(&state);
        let note = store.note_mut(&id).ok_or("노트를 찾을 수 없습니다")?;
        note.collapsed = collapsed;
        let size = (note.width, note.height);
        store.mark_notes();
        store.flush();
        size
    };
    windowing::apply_collapsed(&app, &id, collapsed, width, height);
    Ok(())
}

#[tauri::command]
fn set_note_font_scale(app: AppHandle, id: String, font_scale: f64) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut store = lock_store(&state);
    let note = store.note_mut(&id).ok_or("노트를 찾을 수 없습니다")?;
    note.font_scale = font_scale.clamp(0.85, 1.45);
    store.mark_notes();
    store.flush();
    Ok(())
}

#[tauri::command]
fn set_note_opacity(app: AppHandle, id: String, opacity: f64) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut store = lock_store(&state);
    let note = store.note_mut(&id).ok_or("노트를 찾을 수 없습니다")?;
    note.opacity = clamp_opacity(opacity);
    store.mark_notes();
    store.flush();
    Ok(())
}

#[tauri::command]
fn open_note_list(app: AppHandle) {
    windowing::open_finder(&app);
}

pub fn set_startup(app: &AppHandle, enabled: bool) -> Result<(), String> {
    set_login(app, enabled)?;
    {
        let state = app.state::<AppState>();
        let mut store = lock_store(&state);
        store.settings.open_on_login = enabled;
        store.mark_settings();
        store.flush();
    }
    tray::refresh(app);
    Ok(())
}

#[tauri::command]
fn set_reminder(app: AppHandle, id: String, remind_at_ms: Option<i64>) -> Result<(), String> {
    {
        let state = app.state::<AppState>();
        let mut store = lock_store(&state);
        let note = store.note_mut(&id).ok_or("노트를 찾을 수 없습니다")?;
        note.remind_at_ms = remind_at_ms;
        store.mark_notes();
        store.flush();
    }
    changed(&app);
    Ok(())
}

#[tauri::command]
fn dismiss_note(app: AppHandle, id: String) -> Result<(), String> {
    windowing::dismiss_note(&app, &id);
    Ok(())
}

#[tauri::command]
fn archive_note(app: AppHandle, id: String) -> Result<(), String> {
    {
        let state = app.state::<AppState>();
        let mut store = lock_store(&state);
        store.archive(&id)?;
    }
    windowing::close_note_window(&app, &id);
    changed(&app);
    Ok(())
}

#[tauri::command]
fn restore_note(app: AppHandle, id: String) -> Result<(), String> {
    {
        let state = app.state::<AppState>();
        let mut store = lock_store(&state);
        store.restore(&id)?;
    }
    windowing::open_note(&app, &id);
    changed(&app);
    Ok(())
}

#[tauri::command]
fn destroy_note(app: AppHandle, id: String) -> Result<(), String> {
    {
        let state = app.state::<AppState>();
        let mut store = lock_store(&state);
        store.destroy_note(&id)?;
    }
    windowing::close_note_window(&app, &id);
    changed(&app);
    Ok(())
}

#[tauri::command]
fn empty_trash(app: AppHandle) -> Result<(), String> {
    {
        let state = app.state::<AppState>();
        let mut store = lock_store(&state);
        store.empty_trash()?;
    }
    changed(&app);
    Ok(())
}

#[tauri::command]
fn duplicate_note(app: AppHandle, id: String) -> Result<String, String> {
    let new_id = {
        let state = app.state::<AppState>();
        let mut store = lock_store(&state);
        store.duplicate(&id)?
    };
    windowing::open_note(&app, &new_id);
    changed(&app);
    Ok(new_id)
}

#[tauri::command]
fn create_note(app: AppHandle) -> String {
    windowing::create_note(&app)
}

#[tauri::command]
fn focus_note(app: AppHandle, id: String) -> Result<(), String> {
    let exists = {
        let state = app.state::<AppState>();
        let store = lock_store(&state);
        store.note(&id).is_some()
    };
    if !exists {
        return Err("노트를 찾을 수 없습니다".into());
    }
    windowing::show_note(&app, &id);
    Ok(())
}

#[tauri::command]
fn save_asset(
    app: AppHandle,
    note_id: String,
    mime: String,
    data_base64: String,
) -> Result<AssetDto, String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data_base64.trim())
        .map_err(|_| "이미지 데이터를 읽지 못했습니다".to_string())?;
    let state = app.state::<AppState>();
    let store = lock_store(&state);
    let saved = store.add_asset(&note_id, &mime, bytes)?;
    drop(store);
    changed(&app);
    Ok(saved)
}

#[tauri::command]
fn note_assets(app: AppHandle, note_id: String) -> Result<Vec<AssetDto>, String> {
    let state = app.state::<AppState>();
    let store = lock_store(&state);
    store.assets_for_note(&note_id)
}

#[tauri::command]
fn read_clipboard() -> ClipDto {
    clipboard_payload()
}

#[tauri::command]
fn get_settings(app: AppHandle) -> SettingsDto {
    let state = app.state::<AppState>();
    let store = lock_store(&state);
    store.settings_dto()
}

#[tauri::command]
fn pick_data_dir(app: AppHandle) -> Result<Option<String>, String> {
    let picked = app
        .dialog()
        .file()
        .set_title("노트 데이터 폴더")
        .blocking_pick_folder();
    match picked {
        None => Ok(None),
        Some(path) => {
            let buf = path.into_path().map_err(|err| err.to_string())?;
            Ok(Some(buf.to_string_lossy().into_owned()))
        }
    }
}

#[tauri::command]
fn change_data_dir(app: AppHandle, path: String, mode: String) -> Result<SettingsDto, String> {
    let target = std::path::PathBuf::from(path.trim());
    if target.as_os_str().is_empty() {
        return Err("폴더를 선택하세요".into());
    }
    {
        let state = app.state::<AppState>();
        let mut store = lock_store(&state);
        match mode.as_str() {
            "move" => store.move_data_dir(target)?,
            "switch" => store.switch_data_dir(target)?,
            _ => return Err("알 수 없는 폴더 변경입니다".into()),
        }
    }
    windowing::reload_open_notes(&app);
    let state = app.state::<AppState>();
    let store = lock_store(&state);
    Ok(store.settings_dto())
}

#[tauri::command]
fn set_open_on_login(app: AppHandle, enabled: bool) -> Result<(), String> {
    set_startup(&app, enabled)
}

#[tauri::command]
fn set_restore_on_start(app: AppHandle, enabled: bool) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut store = lock_store(&state);
    store.settings.restore_on_start = enabled;
    store.mark_settings();
    store.flush();
    Ok(())
}

#[tauri::command]
fn set_default_color(app: AppHandle, color: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut store = lock_store(&state);
    store.settings.default_color = color_or_default(&color);
    store.mark_settings();
    store.flush();
    Ok(())
}

#[tauri::command]
fn open_data_dir(app: AppHandle) -> Result<(), String> {
    let dir = {
        let state = app.state::<AppState>();
        let store = lock_store(&state);
        store.data_dir().to_path_buf()
    };
    std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    opener::open(&dir).map_err(|err| err.to_string())
}

#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    let allowed = url.starts_with("https://") || url.starts_with("http://") || url.starts_with("mailto:");
    if !allowed || url.chars().any(|ch| ch.is_control()) {
        return Err("허용되지 않은 주소입니다".into());
    }
    opener::open(url).map_err(|err| err.to_string())
}

fn set_login(app: &AppHandle, enabled: bool) -> Result<(), String> {
    let auto = app.autolaunch();
    if enabled {
        auto.enable().map_err(|err| err.to_string())
    } else {
        auto.disable().map_err(|err| err.to_string())
    }
}

fn clipboard_payload() -> ClipDto {
    let empty = ClipDto {
        kind: "empty".into(),
        mime: String::new(),
        data_base64: String::new(),
        text: String::new(),
    };
    let mut clipboard = match arboard::Clipboard::new() {
        Ok(clipboard) => clipboard,
        Err(_) => return empty,
    };
    if let Ok(image) = clipboard.get_image() {
        let width = image.width as u32;
        let height = image.height as u32;
        let expected = (width as usize).saturating_mul(height as usize).saturating_mul(4);
        if width > 0 && height > 0 && image.bytes.len() == expected {
            let mut png = Vec::new();
            if PngEncoder::new(&mut png)
                .write_image(&image.bytes, width, height, ExtendedColorType::Rgba8)
                .is_ok()
            {
                return ClipDto {
                    kind: "image".into(),
                    mime: "image/png".into(),
                    data_base64: base64::engine::general_purpose::STANDARD.encode(png),
                    text: String::new(),
                };
            }
        }
    }
    if let Ok(text) = clipboard.get_text() {
        if !text.is_empty() {
            return ClipDto {
                kind: "text".into(),
                mime: String::new(),
                data_base64: String::new(),
                text,
            };
        }
    }
    empty
}
