use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, PhysicalSize, WebviewUrl,
    WebviewWindow, WebviewWindowBuilder, WindowEvent,
};

use crate::model::{now_ms, Note, DEFAULT_HEIGHT, DEFAULT_WIDTH, COLLAPSED_HEIGHT};
use crate::state::{lock_set, lock_store, AppState};
use crate::tray;

pub fn note_label(id: &str) -> String {
    format!("n-{id}")
}

pub fn create_note(app: &AppHandle) -> String {
    let index = {
        let state = app.state::<AppState>();
        let store = lock_store(&state);
        store.notes.len()
    };
    let (x, y) = fresh_position(app, index);
    let id = {
        let state = app.state::<AppState>();
        let mut store = lock_store(&state);
        let color = store.settings.default_color.clone();
        let now = now_ms();
        let id = crate::model::new_id();
        store.notes.push(Note {
            id: id.clone(),
            title: String::new(),
            body_html: String::new(),
            color,
            x,
            y,
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
            pinned: true,
            collapsed: false,
            is_open: true,
            archived: false,
            font_scale: 1.0,
            opacity: 1.0,
            created_at_ms: now,
            updated_at_ms: now,
            remind_at_ms: None,
        });
        store.mark_notes();
        store.flush();
        id
    };
    open_note(app, &id);
    announce(app);
    id
}

pub fn open_note(app: &AppHandle, id: &str) {
    let label = note_label(id);
    if let Some(window) = app.get_webview_window(&label) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }
    let app = app.clone();
    let id = id.to_string();
    std::thread::spawn(move || {
        if let Err(err) = build_note(&app, &id) {
            eprintln!("note window: {err}");
        }
    });
}

pub fn show_note(app: &AppHandle, id: &str) {
    {
        let state = app.state::<AppState>();
        let mut store = lock_store(&state);
        let Some(note) = store.note_mut(id) else {
            return;
        };
        note.archived = false;
        note.is_open = true;
        store.mark_notes();
        store.flush();
    }
    open_note(app, id);
    announce(app);
}

pub fn show_open_notes(app: &AppHandle) {
    let ids: Vec<String> = {
        let state = app.state::<AppState>();
        let store = lock_store(&state);
        store
            .notes
            .iter()
            .filter(|note| note.is_open && !note.archived)
            .map(|note| note.id.clone())
            .collect()
    };
    for id in ids {
        open_note(app, &id);
    }
}

pub fn show_all(app: &AppHandle) {
    let ids: Vec<String> = {
        let state = app.state::<AppState>();
        let mut store = lock_store(&state);
        let mut ids = Vec::new();
        for note in &mut store.notes {
            if note.archived {
                continue;
            }
            note.is_open = true;
            ids.push(note.id.clone());
        }
        store.mark_notes();
        store.flush();
        ids
    };
    for id in ids {
        open_note(app, &id);
    }
    announce(app);
}

pub fn hide_all(app: &AppHandle) {
    let ids: Vec<String> = app
        .webview_windows()
        .keys()
        .filter_map(|label| label.strip_prefix("n-"))
        .map(|id| id.to_string())
        .collect();
    {
        let state = app.state::<AppState>();
        let mut store = lock_store(&state);
        for note in &mut store.notes {
            note.is_open = false;
        }
        store.mark_notes();
        store.flush();
    }
    for id in ids {
        destroy_note_window(app, &id);
    }
    announce(app);
}

pub fn toggle_notes(app: &AppHandle) {
    let any_open = app
        .webview_windows()
        .keys()
        .any(|label| label.starts_with("n-"));
    if any_open {
        hide_all(app);
    } else {
        show_all(app);
    }
}

pub fn dismiss_note(app: &AppHandle, id: &str) {
    if let Some(window) = app.get_webview_window(&note_label(id)) {
        let state = app.state::<AppState>();
        let mut store = lock_store(&state);
        if let Some(note) = store.note_mut(id) {
            capture_bounds(&window, note);
            note.is_open = false;
            store.mark_notes();
            store.flush();
        }
    } else {
        let state = app.state::<AppState>();
        let mut store = lock_store(&state);
        if let Some(note) = store.note_mut(id) {
            note.is_open = false;
            store.mark_notes();
            store.flush();
        }
    }
    destroy_note_window(app, id);
    announce(app);
}

pub fn close_note_window(app: &AppHandle, id: &str) {
    destroy_note_window(app, id);
}

pub fn reload_open_notes(app: &AppHandle) {
    let ids_on_screen: Vec<String> = app
        .webview_windows()
        .keys()
        .filter_map(|label| label.strip_prefix("n-").map(|id| id.to_string()))
        .collect();
    for id in &ids_on_screen {
        destroy_note_window(app, id);
    }
    show_open_notes(app);
    announce(app);
}

pub fn open_finder(app: &AppHandle) {
    open_tool(app, "finder", "노트 목록", 440.0, 580.0);
}

pub fn open_settings(app: &AppHandle) {
    open_tool(app, "settings", "설정", 460.0, 660.0);
}

pub fn apply_pinned(app: &AppHandle, id: &str, pinned: bool) {
    if let Some(window) = app.get_webview_window(&note_label(id)) {
        let _ = window.set_always_on_top(pinned);
    }
}

pub fn apply_collapsed(app: &AppHandle, id: &str, collapsed: bool, width: f64, height: f64) {
    if let Some(window) = app.get_webview_window(&note_label(id)) {
        let _ = window.set_resizable(!collapsed);
        let next = if collapsed { COLLAPSED_HEIGHT } else { height };
        let _ = window.set_size(LogicalSize::new(width, next));
    }
}

pub fn set_note_title(app: &AppHandle, id: &str, title: &str) {
    if let Some(window) = app.get_webview_window(&note_label(id)) {
        let label = if title.trim().is_empty() {
            "Stikies"
        } else {
            title
        };
        let _ = window.set_title(label);
    }
}

fn announce(app: &AppHandle) {
    let _ = app.emit("library-changed", ());
    tray::refresh(app);
}

fn destroy_note_window(app: &AppHandle, id: &str) {
    {
        let state = app.state::<AppState>();
        lock_set(&state.allow_close).insert(id.to_string());
    }
    if let Some(window) = app.get_webview_window(&note_label(id)) {
        let _ = window.destroy();
    }
}

fn open_tool(app: &AppHandle, label: &str, title: &str, width: f64, height: f64) {
    if let Some(window) = app.get_webview_window(label) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }
    let app = app.clone();
    let label = label.to_string();
    let title = title.to_string();
    std::thread::spawn(move || {
        let builder = WebviewWindowBuilder::new(
            &app,
            &label,
            WebviewUrl::App("index.html".into()),
        )
        .title(title)
        .inner_size(width, height)
        .min_inner_size(380.0, 460.0)
        .resizable(true)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .center()
        .focused(true);
        #[cfg(debug_assertions)]
        let builder = builder.devtools(true);
        if let Err(err) = builder.build() {
            eprintln!("tool window: {err}");
        }
    });
}

fn build_note(app: &AppHandle, id: &str) -> tauri::Result<()> {
    let label = note_label(id);
    {
        let state = app.state::<AppState>();
        let mut opening = lock_set(&state.opening);
        if app.get_webview_window(&label).is_some() || !opening.insert(id.to_string()) {
            return Ok(());
        }
    }
    let _guard = OpeningGuard {
        app: app.clone(),
        id: id.to_string(),
    };
    let note = {
        let state = app.state::<AppState>();
        let store = lock_store(&state);
        store.note(id).cloned()
    };
    let Some(note) = note else {
        return Ok(());
    };
    let index = {
        let state = app.state::<AppState>();
        let store = lock_store(&state);
        store
            .notes
            .iter()
            .position(|item| item.id == id)
            .unwrap_or(0)
    };
    let (x, y) = resolved_position(app, &note, index);
    {
        let state = app.state::<AppState>();
        let mut store = lock_store(&state);
        if let Some(saved) = store.note_mut(id) {
            if !saved.x.is_finite() || !saved.y.is_finite() {
                saved.x = x;
                saved.y = y;
                store.mark_notes();
                store.flush();
            }
        }
    }
    let height = if note.collapsed {
        COLLAPSED_HEIGHT
    } else {
        note.height
    };
    let title = if note.title.trim().is_empty() {
        "Stikies".to_string()
    } else {
        note.title.clone()
    };
    let builder = WebviewWindowBuilder::new(app, &label, WebviewUrl::App("index.html".into()))
        .title(title)
        .inner_size(note.width, height)
        .min_inner_size(280.0, COLLAPSED_HEIGHT)
        .position(x, y)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .skip_taskbar(true)
        .always_on_top(note.pinned)
        .resizable(!note.collapsed)
        .focused(true)
        .visible(true);
    #[cfg(debug_assertions)]
    let builder = builder.devtools(true);
    let window = builder.build()?;
    attach_note_events(window, id.to_string());
    Ok(())
}

fn attach_note_events(window: WebviewWindow, id: String) {
    let app = window.app_handle().clone();
    window.on_window_event(move |event| match event {
        WindowEvent::Moved(pos) => remember_position(&app, &id, *pos),
        WindowEvent::Resized(size) => remember_size(&app, &id, *size),
        WindowEvent::CloseRequested { api, .. } => {
            let state = app.state::<AppState>();
            let allowed = lock_set(&state.allow_close).contains(&id);
            if allowed {
                return;
            }
            api.prevent_close();
            let app = app.clone();
            let id = id.clone();
            std::thread::spawn(move || dismiss_note(&app, &id));
        }
        _ => {}
    });
}

fn remember_position(app: &AppHandle, id: &str, pos: PhysicalPosition<i32>) {
    let scale = scale_of(app, id);
    let x = pos.x as f64 / scale;
    let y = pos.y as f64 / scale;
    let state = app.state::<AppState>();
    let mut store = lock_store(&state);
    let Some(note) = store.note_mut(id) else {
        return;
    };
    if (note.x - x).abs() < 0.5 && (note.y - y).abs() < 0.5 {
        return;
    }
    note.x = x;
    note.y = y;
    store.mark_notes();
}

fn remember_size(app: &AppHandle, id: &str, size: PhysicalSize<u32>) {
    let scale = scale_of(app, id);
    let width = size.width as f64 / scale;
    let height = size.height as f64 / scale;
    let state = app.state::<AppState>();
    let mut store = lock_store(&state);
    let Some(note) = store.note_mut(id) else {
        return;
    };
    if !width.is_finite() || width < 80.0 {
        return;
    }
    let height_changed = !note.collapsed && (note.height - height).abs() >= 0.5 && height > 80.0;
    let width_changed = (note.width - width).abs() >= 0.5;
    if !height_changed && !width_changed {
        return;
    }
    note.width = width;
    if height_changed {
        note.height = height;
    }
    store.mark_notes();
}

fn capture_bounds(window: &WebviewWindow, note: &mut Note) {
    let scale = window.scale_factor().unwrap_or(1.0).max(0.01);
    if let Ok(pos) = window.outer_position() {
        note.x = pos.x as f64 / scale;
        note.y = pos.y as f64 / scale;
    }
    if let Ok(size) = window.inner_size() {
        let width = size.width as f64 / scale;
        let height = size.height as f64 / scale;
        if width > 80.0 {
            note.width = width;
        }
        if !note.collapsed && height > 80.0 {
            note.height = height;
        }
    }
}

fn scale_of(app: &AppHandle, id: &str) -> f64 {
    app.get_webview_window(&note_label(id))
        .and_then(|window| window.scale_factor().ok())
        .unwrap_or(1.0)
        .max(0.01)
}

fn resolved_position(app: &AppHandle, note: &Note, index: usize) -> (f64, f64) {
    if note.x.is_finite() && note.y.is_finite() && on_any_monitor(app, note.x, note.y, note.width) {
        return (note.x, note.y);
    }
    fresh_position(app, index)
}

fn fresh_position(app: &AppHandle, index: usize) -> (f64, f64) {
    let shift = (index % 8) as f64 * 26.0;
    if let Some((mx, my, mw, mh)) = primary_work_area(app) {
        if index == 0 {
            return (
                mx + (mw - DEFAULT_WIDTH) / 2.0,
                (my + (mh - DEFAULT_HEIGHT) / 2.0 - 24.0).max(my + 16.0),
            );
        }
        return (
            mx + mw - DEFAULT_WIDTH - 40.0 + shift,
            my + 68.0 + shift,
        );
    }
    (96.0 + shift, 72.0 + shift)
}

fn on_any_monitor(app: &AppHandle, x: f64, y: f64, width: f64) -> bool {
    let Ok(monitors) = app.available_monitors() else {
        return true;
    };
    if monitors.is_empty() {
        return true;
    }
    monitors.iter().any(|monitor| {
        let scale = monitor.scale_factor().max(0.01);
        let mx = monitor.position().x as f64 / scale;
        let my = monitor.position().y as f64 / scale;
        let mw = monitor.size().width as f64 / scale;
        let mh = monitor.size().height as f64 / scale;
        x < mx + mw - 40.0 && x + width > mx + 20.0 && y < my + mh - 40.0 && y + 36.0 > my
    })
}

fn primary_work_area(app: &AppHandle) -> Option<(f64, f64, f64, f64)> {
    let monitor = app.primary_monitor().ok().flatten()?;
    let scale = monitor.scale_factor().max(0.01);
    let pos = monitor.position();
    let size = monitor.size();
    Some((
        pos.x as f64 / scale,
        pos.y as f64 / scale,
        size.width as f64 / scale,
        size.height as f64 / scale,
    ))
}

struct OpeningGuard {
    app: AppHandle,
    id: String,
}

impl Drop for OpeningGuard {
    fn drop(&mut self) {
        let state = self.app.state::<AppState>();
        lock_set(&state.opening).remove(&self.id);
    }
}
