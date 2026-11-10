use serde::Serialize;

pub const COLORS: &[&str] = &[
    "sun", "peach", "rose", "leaf", "sky", "lilac", "sand", "ink",
];

pub const DEFAULT_WIDTH: f64 = 360.0;
pub const DEFAULT_HEIGHT: f64 = 460.0;
pub const COLLAPSED_HEIGHT: f64 = 108.0;

#[derive(Clone, Debug)]
pub struct Settings {
    pub data_dir: String,
    pub open_on_login: bool,
    pub restore_on_start: bool,
    pub default_color: String,
    pub welcome_done: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            data_dir: String::new(),
            open_on_login: false,
            restore_on_start: true,
            default_color: "sun".into(),
            welcome_done: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Note {
    pub id: String,
    pub title: String,
    pub body_html: String,
    pub color: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub pinned: bool,
    pub collapsed: bool,
    pub is_open: bool,
    pub archived: bool,
    pub font_scale: f64,
    pub opacity: f64,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    pub remind_at_ms: Option<i64>,
}

#[derive(Clone, Debug)]
pub struct Asset {
    pub id: String,
    pub note_id: String,
    pub mime: String,
    pub bytes: Vec<u8>,
    pub created_at_ms: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteDto {
    pub id: String,
    pub title: String,
    pub body_html: String,
    pub color: String,
    pub pinned: bool,
    pub collapsed: bool,
    pub is_open: bool,
    pub archived: bool,
    pub font_scale: f64,
    pub opacity: f64,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    pub remind_at_ms: Option<i64>,
}

impl From<&Note> for NoteDto {
    fn from(note: &Note) -> Self {
        Self {
            id: note.id.clone(),
            title: note.title.clone(),
            body_html: note.body_html.clone(),
            color: note.color.clone(),
            pinned: note.pinned,
            collapsed: note.collapsed,
            is_open: note.is_open,
            archived: note.archived,
            font_scale: note.font_scale,
            opacity: note.opacity,
            created_at_ms: note.created_at_ms,
            updated_at_ms: note.updated_at_ms,
            remind_at_ms: note.remind_at_ms,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteSummary {
    pub id: String,
    pub title: String,
    pub snippet: String,
    pub color: String,
    pub pinned: bool,
    pub is_open: bool,
    pub archived: bool,
    pub updated_at_ms: i64,
    pub remind_at_ms: Option<i64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetDto {
    pub id: String,
    pub mime: String,
    pub data_base64: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsDto {
    pub data_dir: String,
    pub open_on_login: bool,
    pub restore_on_start: bool,
    pub default_color: String,
    pub note_count: usize,
    pub notes_bytes: u64,
    pub assets_bytes: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipDto {
    pub kind: String,
    pub mime: String,
    pub data_base64: String,
    pub text: String,
}

pub fn color_or_default(color: &str) -> String {
    if COLORS.contains(&color) {
        color.to_string()
    } else {
        "sun".into()
    }
}

pub fn snippet_from_html(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                out.push(' ');
            }
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    let collapsed = out.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed.chars().take(96).collect()
}

pub fn clamp_opacity(value: f64) -> f64 {
    if value.is_finite() {
        value.clamp(0.4, 1.0)
    } else {
        1.0
    }
}

pub fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}
