use std::cell::RefCell;
use std::f32::consts::PI;
use std::ffi::c_void;
use std::mem::{size_of, zeroed};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::process::Command;
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, Ordering};
use std::sync::{Arc, OnceLock, RwLock};
use std::time::Instant;

use serde_json::json;
use windows_sys::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, BitBlt, CreateCompatibleDC, CreateDIBSection, CreateFontW, CreatePen,
    CreateSolidBrush, DeleteDC, DeleteObject, DrawTextW, EndPaint, FillRect, GdiFlush,
    GetMonitorInfoW, InvalidateRect, MonitorFromPoint, RoundRect, SelectObject, SetBkColor,
    SetBkMode, SetTextColor, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, CLEARTYPE_QUALITY,
    CLIP_DEFAULT_PRECIS, DEFAULT_CHARSET, DEFAULT_PITCH, DIB_RGB_COLORS, DT_CALCRECT, DT_CENTER,
    DT_END_ELLIPSIS, DT_LEFT, DT_NOPREFIX, DT_RIGHT, DT_SINGLELINE, DT_VCENTER, FF_DONTCARE, HDC,
    HFONT, MONITORINFO, MONITOR_DEFAULTTONEAREST, OUT_DEFAULT_PRECIS, PAINTSTRUCT, PS_SOLID,
    SRCCOPY, TRANSPARENT,
};
use windows_sys::Win32::System::Com::CoTaskMemFree;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, ReleaseCapture, SetCapture, SetFocus, TrackMouseEvent, TME_LEAVE,
    TRACKMOUSEEVENT, VK_CONTROL, VK_DOWN, VK_ESCAPE, VK_MENU, VK_NEXT, VK_PRIOR, VK_RETURN,
    VK_SPACE, VK_TAB, VK_UP,
};
use windows_sys::Win32::UI::Shell::{
    SHBrowseForFolderW, SHGetPathFromIDListW, BIF_NEWDIALOGSTYLE, BIF_RETURNONLYFSDIRS, BROWSEINFOW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallWindowProcW, CreateWindowExW, DefWindowProcW, GetClientRect, GetCursorPos, GetWindowRect,
    GetWindowTextLengthW, GetWindowTextW, IsWindow, LoadCursorW, RegisterClassExW, SendMessageW,
    SetCursor, SetForegroundWindow, SetWindowLongPtrW, SetWindowPos, SetWindowTextW, ShowWindow,
    CS_HREDRAW, CS_VREDRAW, EN_CHANGE, GWLP_WNDPROC, HTCAPTION, HWND_TOPMOST, IDC_ARROW, IDC_HAND,
    SWP_NOACTIVATE, SWP_SHOWWINDOW, SW_HIDE, SW_SHOW, WA_INACTIVE, WM_ACTIVATE, WM_APP, WM_CHAR,
    WM_COMMAND, WM_CTLCOLOREDIT, WM_ERASEBKGND, WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP,
    WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_NCLBUTTONDOWN, WM_PAINT, WM_SETFONT, WM_SYSCHAR,
    WM_SYSKEYDOWN, WNDCLASSEXW, WS_CHILD, WS_CLIPCHILDREN, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
    WS_POPUP, WS_VISIBLE,
};

const WM_MOUSELEAVE: u32 = 0x02A3;

use crate::aegp_host::{dispatch_ae_script, get_ae_main_hwnd};
use crate::config::{
    enrich_effects, get_app_data_dir, load_initial_effects, load_settings, load_usage,
    save_settings, save_usage, set_custom_app_data_dir, AppSettings, EffectsCacheFile,
    IndexedEffect, RawAeEffect, UsageStats,
};
use crate::presets::{compute_presets_dir_stamp, scan_all_presets, IndexedPreset};
use crate::search::{
    build_catalog, enrich_used_effects, filter_catalog, filter_used_effects, CatalogItem,
    CatalogKind, ConsoleMode, EnrichedUsedEffect, FxTypeFilter, RawUsedEffect, UsedScope,
};

pub const WM_APP_TOGGLE_FX: u32 = WM_APP + 10;
pub const WM_APP_TOGGLE_USED: u32 = WM_APP + 11;
pub const WM_APP_OPEN_FX: u32 = WM_APP + 12;
pub const WM_APP_OPEN_USED: u32 = WM_APP + 13;
pub const WM_APP_HIDE_POPUP: u32 = WM_APP + 14;
pub const WM_APP_RECORD_KEY: u32 = WM_APP + 15;

const BASE_WIDTH: i32 = 600;
const MAX_VISIBLE_ROWS: usize = 8;

const fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
    (r as u32) | ((g as u32) << 8) | ((b as u32) << 16)
}

const COL_BG_SHELL: COLORREF = rgb(23, 25, 32);
const COL_BG_TOPBAR: COLORREF = rgb(16, 18, 24);
const COL_BG_SEARCH: COLORREF = rgb(30, 34, 44);
const COL_BG_FOOTER: COLORREF = rgb(15, 17, 22);
const COL_BG_HOVER: COLORREF = rgb(34, 39, 50);
const COL_BG_TOPBTN_HOVER: COLORREF = rgb(40, 46, 60);
const COL_BORDER_TOPBTN_HOVER: COLORREF = rgb(94, 106, 132);
const COL_BG_SEL_FX: COLORREF = rgb(24, 54, 80);
const COL_BG_SEL_FX_HOV: COLORREF = rgb(30, 68, 98);
const COL_BG_SEL_USED: COLORREF = rgb(46, 35, 78);
const COL_BG_SEL_USED_HOV: COLORREF = rgb(58, 45, 96);
const COL_BG_RECORDING: COLORREF = rgb(68, 46, 14);
const COL_BORDER_FX: COLORREF = rgb(48, 128, 180);
const COL_BORDER_USED: COLORREF = rgb(124, 92, 204);
const COL_BORDER_SUBTLE: COLORREF = rgb(45, 50, 64);

const COL_SCROLL_TRACK: COLORREF = rgb(19, 21, 28);
const COL_SCROLL_THUMB: COLORREF = rgb(68, 76, 94);
const COL_SCROLL_THUMB_ACTIVE: COLORREF = rgb(126, 138, 166);

const COL_TEXT_MAIN: COLORREF = rgb(242, 245, 250);
const COL_TEXT_MUTED: COLORREF = rgb(142, 151, 170);
const COL_TEXT_DIM: COLORREF = rgb(98, 106, 124);
const COL_CYAN: COLORREF = rgb(56, 189, 248);
const COL_VIOLET: COLORREF = rgb(196, 181, 253);
const COL_AMBER: COLORREF = rgb(251, 191, 36);
const COL_GREEN: COLORREF = rgb(52, 211, 153);
const COL_RED_BG: COLORREF = rgb(72, 26, 30);
const COL_RED_TEXT: COLORREF = rgb(252, 165, 165);

const AE_LABEL_COLORS: [COLORREF; 17] = [
    rgb(100, 116, 139),
    rgb(239, 68, 68),
    rgb(234, 179, 8),
    rgb(103, 232, 249),
    rgb(249, 168, 212),
    rgb(196, 181, 253),
    rgb(253, 186, 116),
    rgb(167, 243, 208),
    rgb(96, 165, 250),
    rgb(74, 222, 128),
    rgb(168, 85, 247),
    rgb(249, 115, 22),
    rgb(161, 98, 7),
    rgb(236, 72, 153),
    rgb(6, 182, 212),
    rgb(214, 211, 209),
    rgb(21, 128, 61),
];

#[derive(Debug, Clone, Copy)]
pub struct ParsedShortcut {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub vk: u32,
}

pub fn is_modifier_vk(vk: u32) -> bool {
    matches!(
        vk,
        0x10 | 0x11 | 0x12 | 0xA0 | 0xA1 | 0xA2 | 0xA3 | 0xA4 | 0xA5 | 0x5B | 0x5C
    )
}

pub fn vk_to_key_name(vk: u32) -> Option<&'static str> {
    match vk {
        0x20 => Some("Space"),
        0x09 => Some("Tab"),
        0xC0 => Some("`"),
        0xBD => Some("-"),
        0xBB => Some("="),
        0xDB => Some("["),
        0xDD => Some("]"),
        0xDC => Some("\\"),
        0xBA => Some(";"),
        0xDE => Some("'"),
        0xBC => Some(","),
        0xBE => Some("."),
        0xBF => Some("/"),
        0x70 => Some("F1"),
        0x71 => Some("F2"),
        0x72 => Some("F3"),
        0x73 => Some("F4"),
        0x74 => Some("F5"),
        0x75 => Some("F6"),
        0x76 => Some("F7"),
        0x77 => Some("F8"),
        0x78 => Some("F9"),
        0x79 => Some("F10"),
        0x7A => Some("F11"),
        0x7B => Some("F12"),
        0x30 => Some("0"),
        0x31 => Some("1"),
        0x32 => Some("2"),
        0x33 => Some("3"),
        0x34 => Some("4"),
        0x35 => Some("5"),
        0x36 => Some("6"),
        0x37 => Some("7"),
        0x38 => Some("8"),
        0x39 => Some("9"),
        0x41 => Some("A"),
        0x42 => Some("B"),
        0x43 => Some("C"),
        0x44 => Some("D"),
        0x45 => Some("E"),
        0x46 => Some("F"),
        0x47 => Some("G"),
        0x48 => Some("H"),
        0x49 => Some("I"),
        0x4A => Some("J"),
        0x4B => Some("K"),
        0x4C => Some("L"),
        0x4D => Some("M"),
        0x4E => Some("N"),
        0x4F => Some("O"),
        0x50 => Some("P"),
        0x51 => Some("Q"),
        0x52 => Some("R"),
        0x53 => Some("S"),
        0x54 => Some("T"),
        0x55 => Some("U"),
        0x56 => Some("V"),
        0x57 => Some("W"),
        0x58 => Some("X"),
        0x59 => Some("Y"),
        0x5A => Some("Z"),
        _ => None,
    }
}

pub fn format_shortcut_spec(ctrl: bool, shift: bool, alt: bool, vk: u32) -> Option<String> {
    let key_name = vk_to_key_name(vk)?;
    let is_fn_key = (0x70..=0x7B).contains(&vk);
    if !ctrl && !shift && !alt && !is_fn_key {
        return None;
    }
    let mut parts = Vec::with_capacity(4);
    if ctrl {
        parts.push("Ctrl");
    }
    if shift {
        parts.push("Shift");
    }
    if alt {
        parts.push("Alt");
    }
    parts.push(key_name);
    Some(parts.join("+"))
}

pub fn parse_shortcut(spec: &str) -> ParsedShortcut {
    let mut ctrl = false;
    let mut shift = false;
    let mut alt = false;
    let mut vk = VK_SPACE as u32;

    for part in spec.split('+').map(|s| s.trim()) {
        let low = part.to_ascii_lowercase();
        match low.as_str() {
            "ctrl" | "control" => ctrl = true,
            "shift" => shift = true,
            "alt" => alt = true,
            "space" => vk = 0x20,
            "tab" => vk = 0x09,
            "`" | "tilde" => vk = 0xC0,
            "-" => vk = 0xBD,
            "=" => vk = 0xBB,
            "[" => vk = 0xDB,
            "]" => vk = 0xDD,
            "\\" => vk = 0xDC,
            ";" => vk = 0xBA,
            "'" => vk = 0xDE,
            "," => vk = 0xBC,
            "." => vk = 0xBE,
            "/" => vk = 0xBF,
            "f1" => vk = 0x70,
            "f2" => vk = 0x71,
            "f3" => vk = 0x72,
            "f4" => vk = 0x73,
            "f5" => vk = 0x74,
            "f6" => vk = 0x75,
            "f7" => vk = 0x76,
            "f8" => vk = 0x77,
            "f9" => vk = 0x78,
            "f10" => vk = 0x79,
            "f11" => vk = 0x7A,
            "f12" => vk = 0x7B,
            s if s.len() == 1 => {
                let b = s.as_bytes()[0];
                if b.is_ascii_alphabetic() {
                    vk = b.to_ascii_uppercase() as u32;
                } else if b.is_ascii_digit() {
                    vk = b as u32;
                }
            }
            _ => {}
        }
    }
    ParsedShortcut {
        ctrl,
        shift,
        alt,
        vk,
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ShortcutPair {
    pub fx: ParsedShortcut,
    pub used: ParsedShortcut,
}

static SHARED_SHORTCUTS: OnceLock<Arc<RwLock<ShortcutPair>>> = OnceLock::new();

pub fn get_or_init_shortcuts() -> Arc<RwLock<ShortcutPair>> {
    SHARED_SHORTCUTS
        .get_or_init(|| {
            let s = load_settings();
            Arc::new(RwLock::new(ShortcutPair {
                fx: parse_shortcut(&s.hotkey_fx),
                used: parse_shortcut(&s.hotkey_used),
            }))
        })
        .clone()
}

fn sync_shared_shortcuts(fx: ParsedShortcut, used: ParsedShortcut) {
    if let Ok(mut w) = get_or_init_shortcuts().write() {
        w.fx = fx;
        w.used = used;
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum HitAction {
    TabFx,
    TabUsed,
    ToggleFilterOrScope,
    ToggleSettings,
    ScrollbarTrack,
    ScrollbarThumb,
    RowClick(usize),
    StarClick(usize),
    ToggleLanguage,
    StartRecordFxHotkey,
    StartRecordUsedHotkey,
    ToggleDefaultScope,
    RescanCatalog,
    OpenConfigFolder,
    ChangeConfigFolder,
    DragHeader,
}

#[derive(Clone, Copy)]
struct HitBox {
    rect: RECT,
    action: HitAction,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum HoverTarget {
    None,
    TabFx,
    TabUsed,
    FilterOrScope,
    SettingsBtn,
    ScrollbarThumb,
    Row(usize),
    SettingsItem(usize),
}

static POPUP_HWND_ATOMIC: AtomicIsize = AtomicIsize::new(0);
static EDIT_HWND_ATOMIC: AtomicIsize = AtomicIsize::new(0);
static SEARCH_BG_BRUSH_ATOMIC: AtomicIsize = AtomicIsize::new(0);
static EDIT_FONT_ATOMIC: AtomicIsize = AtomicIsize::new(0);
static CURRENT_DPI_ATOMIC: AtomicU32 = AtomicU32::new(96);
static POPUP_VISIBLE: AtomicBool = AtomicBool::new(false);
static SUPPRESS_EDIT_CHANGE: AtomicBool = AtomicBool::new(false);
static MODAL_DIALOG_OPEN: AtomicBool = AtomicBool::new(false);
static EDIT_ORIG_WNDPROC: AtomicIsize = AtomicIsize::new(0);
static RECORDING_TARGET: AtomicU32 = AtomicU32::new(0);

pub fn is_modal_dialog_open() -> bool {
    MODAL_DIALOG_OPEN.load(Ordering::SeqCst)
}

pub fn get_recording_target() -> u32 {
    RECORDING_TARGET.load(Ordering::SeqCst)
}

pub fn clear_recording_target() {
    RECORDING_TARGET.store(0, Ordering::SeqCst);
}

struct UiState {
    opened_at: Instant,
    mode: ConsoleMode,
    fx_filter: FxTypeFilter,
    used_scope: UsedScope,

    settings: AppSettings,
    fx_shortcut: ParsedShortcut,
    used_shortcut: ParsedShortcut,
    usage: UsageStats,
    effects: Vec<IndexedEffect>,
    effects_synced: bool,
    presets: Vec<IndexedPreset>,
    presets_scanned: bool,
    presets_dir_stamp: u64,
    catalog: Vec<CatalogItem>,
    used_effects: Vec<EnrichedUsedEffect>,

    active_comp_name: String,
    selected_layer_count: i32,
    toast_error: String,

    query: String,
    filtered_indices: Vec<usize>,
    selected_pos: usize,
    scroll_offset: usize,
    hover: HoverTarget,
    tracking_mouse_leave: bool,

    // Scrollbar drag state
    dragging_scrollbar: bool,
    drag_grab_offset_y: i32,
    sb_track_top: i32,
    sb_track_h: i32,
    sb_thumb_h: i32,

    hit_boxes: Vec<HitBox>,
}

thread_local! {
    static UI_STATE: RefCell<Option<UiState>> = const { RefCell::new(None) };
}

fn with_ui<R>(f: impl FnOnce(&mut UiState) -> R) -> Option<R> {
    UI_STATE.with(|cell| {
        let mut borrow = cell.try_borrow_mut().ok()?;
        let st = borrow.as_mut()?;
        Some(f(st))
    })
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn pt_in_rect(r: &RECT, x: i32, y: i32) -> bool {
    x >= r.left && x < r.right && y >= r.top && y < r.bottom
}

pub fn get_popup_hwnd() -> HWND {
    POPUP_HWND_ATOMIC.load(Ordering::SeqCst) as HWND
}

pub fn is_popup_visible() -> bool {
    POPUP_VISIBLE.load(Ordering::SeqCst)
}

fn scale_px(val: i32, dpi: u32) -> i32 {
    ((val as i64 * dpi.max(96) as i64 + 48) / 96) as i32
}

unsafe fn create_ui_font(px_height: i32, weight: i32) -> HFONT {
    CreateFontW(
        -px_height,
        0,
        0,
        0,
        weight,
        0,
        0,
        0,
        DEFAULT_CHARSET as u32,
        OUT_DEFAULT_PRECIS as u32,
        CLIP_DEFAULT_PRECIS as u32,
        CLEARTYPE_QUALITY as u32,
        (DEFAULT_PITCH | FF_DONTCARE) as u32,
        to_wide("Microsoft YaHei UI").as_ptr(),
    )
}

unsafe fn measure_text_width(hdc: HDC, text: &str) -> i32 {
    let w = to_wide(text);
    let mut rc = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    DrawTextW(
        hdc,
        w.as_ptr(),
        (w.len() - 1) as i32,
        &mut rc,
        DT_CALCRECT | DT_SINGLELINE | DT_NOPREFIX,
    );
    rc.right - rc.left
}

// ============================================================================
// High-precision 4x4 Supersampled Vector SDF Icon Renderers (Target & Gear)
// ============================================================================

/// Evaluate SDF of `boxicons--target.svg` (outer ring + 4 cardinal ticks + solid center bullseye) in [-0.5, 0.5]^2.
#[inline]
fn sdf_boxicons_target(nx: f32, ny: f32) -> f32 {
    let ax = nx.abs();
    let ay = ny.abs();
    let r = (ax * ax + ay * ay).sqrt();

    // 1. Solid center circle
    let d_center = r - 0.135;

    // 2. Outer hollow circle ring
    let d_ring = (r - 0.305).abs() - 0.039;

    // 3. Top & bottom vertical crosshair ticks
    let cy_seg = ay.clamp(0.285, 0.445);
    let d_vert = (ax * ax + (ay - cy_seg).powi(2)).sqrt() - 0.038;

    // 4. Left & right horizontal crosshair ticks
    let cx_seg = ax.clamp(0.285, 0.445);
    let d_horiz = ((ax - cx_seg).powi(2) + ay * ay).sqrt() - 0.038;

    d_center.min(d_ring).min(d_vert).min(d_horiz)
}

/// Evaluate SDF of `ant-design--setting-outlined.svg` (6-tooth hollow gear + hollow inner ring) in [-0.5, 0.5]^2.
#[inline]
fn sdf_ant_setting_outlined(nx: f32, ny: f32) -> f32 {
    let r = (nx * nx + ny * ny).sqrt();

    // Fold polar angle into 6 symmetrical 60-degree sectors with tooth #0 pointing straight UP (0, -1)
    let theta = nx.atan2(-ny);
    let sector = PI / 3.0;
    let rel = (theta + PI + sector * 0.5).rem_euclid(sector) - sector * 0.5;
    let ux = r * rel.sin().abs();
    let uy = r * rel.cos();

    // Central gear body disk + rounded trapezoidal tooth
    let d_body = r - 0.305;
    let qx = ux - (0.088 - 0.026);
    let qy = (uy - 0.315).abs() - (0.112 - 0.026);
    let d_tooth = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - 0.026;

    // Smooth fillet union between teeth and body
    let k = 0.030;
    let h = (0.5 + 0.5 * (d_tooth - d_body) / k).clamp(0.0, 1.0);
    let d_gear_solid = d_tooth * (1.0 - h) + d_body * h - k * h * (1.0 - h);

    // Constant-thickness hollow gear outline + hollow inner circle
    let d_outer_stroke = (d_gear_solid + 0.018).abs() - 0.036;
    let d_inner_stroke = (r - 0.132).abs() - 0.035;

    d_outer_stroke.min(d_inner_stroke)
}

unsafe fn draw_sdf_icon(
    bits: *mut u32,
    buf_w: i32,
    buf_h: i32,
    cx: i32,
    cy: i32,
    size_px: i32,
    color: COLORREF,
    sdf_fn: fn(f32, f32) -> f32,
) {
    if bits.is_null() || size_px <= 2 {
        return;
    }
    let src_r = (color & 0xFF) as f32;
    let src_g = ((color >> 8) & 0xFF) as f32;
    let src_b = ((color >> 16) & 0xFF) as f32;

    let half = size_px as f32 * 0.5;
    let inv_size = 1.0 / (size_px as f32);
    let x0 = (cx - (size_px / 2) - 1).max(0);
    let x1 = (cx + (size_px / 2) + 2).min(buf_w - 1);
    let y0 = (cy - (size_px / 2) - 1).max(0);
    let y1 = (cy + (size_px / 2) + 2).min(buf_h - 1);

    // 4x4 subpixel supersampling (16 samples per pixel) for razor-sharp vector anti-aliasing
    const SUB: i32 = 4;
    let step = 1.0 / (SUB as f32);

    for py in y0..=y1 {
        let row_offset = (py * buf_w) as usize;
        for px in x0..=x1 {
            let mut cov = 0.0f32;
            for sy in 0..SUB {
                let fy = (py as f32 + (sy as f32 + 0.5) * step - cy as f32) * inv_size;
                for sx in 0..SUB {
                    let fx = (px as f32 + (sx as f32 + 0.5) * step - cx as f32) * inv_size;
                    if fx.abs() <= 0.52 && fy.abs() <= 0.52 {
                        if sdf_fn(fx, fy) <= 0.0 {
                            cov += 1.0;
                        }
                    }
                }
            }
            let alpha = cov * (1.0 / ((SUB * SUB) as f32));
            if alpha > 0.001 {
                let idx = row_offset + (px as usize);
                let dst = *bits.add(idx);
                // DIB 32-bit BGRA in memory: byte 0 = B, byte 1 = G, byte 2 = R
                let dst_b = (dst & 0xFF) as f32;
                let dst_g = ((dst >> 8) & 0xFF) as f32;
                let dst_r = ((dst >> 16) & 0xFF) as f32;

                let out_r = (dst_r * (1.0 - alpha) + src_r * alpha).round() as u32;
                let out_g = (dst_g * (1.0 - alpha) + src_g * alpha).round() as u32;
                let out_b = (dst_b * (1.0 - alpha) + src_b * alpha).round() as u32;

                *bits.add(idx) = (out_b & 0xFF) | ((out_g & 0xFF) << 8) | ((out_r & 0xFF) << 16);
            }
        }
    }
    let _ = half;
}

// ============================================================================

pub fn try_sync_effects_on_idle() -> bool {
    let already_synced = with_ui(|s| s.effects_synced).unwrap_or(false);
    if already_synced {
        return true;
    }

    if let Ok(val) = dispatch_ae_script("getInstalledEffects", json!({})) {
        if let Some(arr) = val.get("effects") {
            if let Ok(raw_list) = serde_json::from_value::<Vec<RawAeEffect>>(arr.clone()) {
                if !raw_list.is_empty() {
                    let cache = EffectsCacheFile {
                        ae_version: val
                            .get("aeVersion")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        updated_at: 0,
                        effects: raw_list.clone(),
                    };
                    if let Ok(json_str) = serde_json::to_string_pretty(&cache) {
                        let _ = std::fs::write(get_app_data_dir().join("effects-cache.json"), json_str);
                    }

                    let custom_dirs = with_ui(|s| {
                        if s.presets_scanned {
                            None
                        } else {
                            Some(s.settings.custom_preset_dirs.clone())
                        }
                    })
                    .flatten();

                    let scanned_presets = custom_dirs.map(|dirs| scan_all_presets(&dirs));
                    let enriched = enrich_effects(&raw_list);

                    with_ui(|st| {
                        if let Some(pr) = scanned_presets {
                            st.presets = pr;
                            st.presets_scanned = true;
                        }
                        st.effects = enriched;
                        st.effects_synced = true;
                        st.catalog = build_catalog(&st.effects, &st.presets, &st.settings);
                        recompute_filtered(st);
                    });
                    return true;
                }
            }
        }
    }
    false
}

pub unsafe fn ensure_ui_initialized() {
    if POPUP_HWND_ATOMIC.load(Ordering::SeqCst) != 0 {
        return;
    }

    let settings = load_settings();
    let usage = load_usage();
    let (effects, synced) = load_initial_effects();
    let presets = Vec::new();
    let catalog = build_catalog(&effects, &presets, &settings);

    let default_scope = if settings.default_used_scope == "active" {
        UsedScope::ActiveComp
    } else {
        UsedScope::AllProject
    };

    let mut st = UiState {
        opened_at: Instant::now(),
        mode: ConsoleMode::ApplyFx,
        fx_filter: FxTypeFilter::All,
        used_scope: default_scope,
        fx_shortcut: parse_shortcut(&settings.hotkey_fx),
        used_shortcut: parse_shortcut(&settings.hotkey_used),
        settings,
        usage,
        effects,
        effects_synced: synced,
        presets,
        presets_scanned: false,
        presets_dir_stamp: 0,
        catalog,
        used_effects: Vec::new(),
        active_comp_name: String::new(),
        selected_layer_count: 0,
        toast_error: String::new(),
        query: String::new(),
        filtered_indices: Vec::new(),
        selected_pos: 0,
        scroll_offset: 0,
        hover: HoverTarget::None,
        tracking_mouse_leave: false,
        dragging_scrollbar: false,
        drag_grab_offset_y: 0,
        sb_track_top: 0,
        sb_track_h: 1,
        sb_thumb_h: 1,
        hit_boxes: Vec::new(),
    };
    recompute_filtered(&mut st);

    let search_bg_brush = CreateSolidBrush(COL_BG_SEARCH);
    SEARCH_BG_BRUSH_ATOMIC.store(search_bg_brush as isize, Ordering::SeqCst);

    UI_STATE.with(|c| {
        if let Ok(mut b) = c.try_borrow_mut() {
            *b = Some(st);
        }
    });

    let class_name = to_wide("FXSearchAEGPPopupClass");
    let hinstance = GetModuleHandleW(null());

    let wc = WNDCLASSEXW {
        cbSize: size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(popup_wnd_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: hinstance,
        hIcon: null_mut(),
        hCursor: LoadCursorW(null_mut(), IDC_ARROW),
        hbrBackground: null_mut(),
        lpszMenuName: null(),
        lpszClassName: class_name.as_ptr(),
        hIconSm: null_mut(),
    };
    let _ = RegisterClassExW(&wc);

    let popup_hwnd = CreateWindowExW(
        WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
        class_name.as_ptr(),
        to_wide("FXSearch").as_ptr(),
        WS_POPUP | WS_CLIPCHILDREN,
        100,
        100,
        BASE_WIDTH,
        420,
        null_mut(),
        null_mut(),
        hinstance,
        null(),
    );
    POPUP_HWND_ATOMIC.store(popup_hwnd as isize, Ordering::SeqCst);

    let corner_pref: u32 = DWMWCP_ROUND as u32;
    let _ = DwmSetWindowAttribute(
        popup_hwnd,
        DWMWA_WINDOW_CORNER_PREFERENCE as u32,
        &corner_pref as *const u32 as *const c_void,
        size_of::<u32>() as u32,
    );

    let edit_hwnd = CreateWindowExW(
        0,
        to_wide("EDIT").as_ptr(),
        to_wide("").as_ptr(),
        WS_CHILD | WS_VISIBLE | 0x0080,
        58,
        46,
        420,
        26,
        popup_hwnd,
        1001usize as *mut c_void,
        hinstance,
        null(),
    );
    EDIT_HWND_ATOMIC.store(edit_hwnd as isize, Ordering::SeqCst);

    let orig_proc = SetWindowLongPtrW(
        edit_hwnd,
        GWLP_WNDPROC,
        edit_subclass_proc as *const () as usize as isize,
    );
    EDIT_ORIG_WNDPROC.store(orig_proc, Ordering::SeqCst);

    let edit_font = create_ui_font(16, 500);
    EDIT_FONT_ATOMIC.store(edit_font as isize, Ordering::SeqCst);
    SendMessageW(edit_hwnd, WM_SETFONT, edit_font as usize, 1);
}

fn compute_desired_logical_height(st: &UiState) -> i32 {
    let top_h = 38;
    let footer_h = 28;
    let toast_h = if st.toast_error.is_empty() { 0 } else { 28 };

    if st.mode == ConsoleMode::Settings {
        return top_h + 216 + footer_h;
    }

    let search_h = 46;
    let row_h = if st.mode == ConsoleMode::ApplyFx { 38 } else { 46 };
    let visible_rows = st.filtered_indices.len().clamp(1, MAX_VISIBLE_ROWS) as i32;
    let list_pad = 10;
    let empty_extra = if st.filtered_indices.is_empty() { 26 } else { 0 };

    top_h + search_h + toast_h + list_pad + visible_rows * row_h + empty_extra + footer_h
}

fn recompute_filtered(st: &mut UiState) {
    match st.mode {
        ConsoleMode::ApplyFx => {
            st.filtered_indices =
                filter_catalog(&st.catalog, &st.query, st.fx_filter, &st.usage);
        }
        ConsoleMode::UsedEffects => {
            st.filtered_indices =
                filter_used_effects(&st.used_effects, &st.query, st.used_scope);
        }
        ConsoleMode::Settings => {
            st.filtered_indices.clear();
        }
    }
    if st.selected_pos >= st.filtered_indices.len() {
        st.selected_pos = 0;
    }
    ensure_selection_in_scroll(st);
}

fn ensure_selection_in_scroll(st: &mut UiState) {
    if st.filtered_indices.is_empty() {
        st.scroll_offset = 0;
        return;
    }
    if st.selected_pos < st.scroll_offset {
        st.scroll_offset = st.selected_pos;
    } else if st.selected_pos >= st.scroll_offset + MAX_VISIBLE_ROWS {
        st.scroll_offset = st.selected_pos + 1 - MAX_VISIBLE_ROWS;
    }
}

fn sync_data_outside_borrow(mode: ConsoleMode, query_ae_host: bool) {
    let (was_scanned, prev_stamp, custom_dirs, need_effects) = with_ui(|st| {
        (
            st.presets_scanned,
            st.presets_dir_stamp,
            st.settings.custom_preset_dirs.clone(),
            !st.effects_synced,
        )
    })
    .unwrap_or((false, 0, Vec::new(), false));

    let cur_stamp = compute_presets_dir_stamp(&custom_dirs);
    if !was_scanned || cur_stamp != prev_stamp {
        let scanned = scan_all_presets(&custom_dirs);
        with_ui(|st| {
            st.presets = scanned;
            st.presets_scanned = true;
            st.presets_dir_stamp = cur_stamp;
            st.catalog = build_catalog(&st.effects, &st.presets, &st.settings);
        });
    }

    if !query_ae_host {
        return;
    }

    if need_effects {
        let _ = try_sync_effects_on_idle();
    }

    if mode == ConsoleMode::UsedEffects {
        match dispatch_ae_script("scanUsedEffects", json!({})) {
            Ok(val) => {
                let comp_name = val
                    .get("activeCompName")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let raw_items: Vec<RawUsedEffect> = val
                    .get("items")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .unwrap_or_default();
                let enriched = enrich_used_effects(raw_items);
                with_ui(|st| {
                    st.active_comp_name = comp_name;
                    st.used_effects = enriched;
                });
            }
            Err(err) => {
                if !err.contains("无法获取 AEGP") && !err.contains("Esc key held") {
                    with_ui(|st| st.toast_error = err);
                }
            }
        }
    } else if mode == ConsoleMode::ApplyFx {
        if let Ok(val) = dispatch_ae_script("getContext", json!({})) {
            let comp_name = val
                .get("activeCompName")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let sel_cnt = val
                .get("selectedLayerCount")
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as i32;
            with_ui(|st| {
                st.active_comp_name = comp_name;
                st.selected_layer_count = sel_cnt;
            });
        }
    }
}

unsafe fn apply_window_layout(reposition_at_cursor: bool) {
    let popup_hwnd = get_popup_hwnd();
    let edit_hwnd = EDIT_HWND_ATOMIC.load(Ordering::SeqCst) as HWND;
    if popup_hwnd.is_null() || edit_hwnd.is_null() {
        return;
    }

    let Some((mode, logical_h)) = with_ui(|st| {
        if reposition_at_cursor {
            st.opened_at = Instant::now();
        }
        (st.mode, compute_desired_logical_height(st))
    }) else {
        return;
    };

    let mut pt = POINT { x: 400, y: 300 };
    if reposition_at_cursor {
        GetCursorPos(&mut pt);
    } else {
        let mut cur_r: RECT = zeroed();
        GetWindowRect(popup_hwnd, &mut cur_r);
        pt.x = cur_r.left + 20;
        pt.y = cur_r.top + 20;
    }

    let monitor = MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST);
    let mut dpi_x: u32 = 96;
    let mut dpi_y: u32 = 96;
    if !monitor.is_null() {
        let _ = GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y);
    }
    let dpi = dpi_x.max(96);
    let old_dpi = CURRENT_DPI_ATOMIC.swap(dpi, Ordering::SeqCst);
    if dpi != old_dpi {
        let new_font = create_ui_font(scale_px(16, dpi), 500);
        let prev_font = EDIT_FONT_ATOMIC.swap(new_font as isize, Ordering::SeqCst) as HFONT;
        SendMessageW(edit_hwnd, WM_SETFONT, new_font as usize, 1);
        if !prev_font.is_null() {
            DeleteObject(prev_font as _);
        }
    }

    let phys_w = scale_px(BASE_WIDTH, dpi);
    let phys_h = scale_px(logical_h, dpi);

    if mode == ConsoleMode::Settings {
        ShowWindow(edit_hwnd, SW_HIDE);
    } else {
        let edit_x = scale_px(56, dpi);
        let edit_y = scale_px(50, dpi);
        let right_hint_w = scale_px(150, dpi);
        let edit_w = (phys_w - edit_x - right_hint_w - scale_px(12, dpi)).max(120);
        let edit_h = scale_px(24, dpi);
        SetWindowPos(
            edit_hwnd,
            null_mut(),
            edit_x,
            edit_y,
            edit_w,
            edit_h,
            SWP_NOACTIVATE,
        );
        ShowWindow(edit_hwnd, SW_SHOW);
    }

    let (mut target_x, mut target_y) = if reposition_at_cursor {
        (pt.x - scale_px(52, dpi), pt.y - scale_px(58, dpi))
    } else {
        let mut cur_r: RECT = zeroed();
        GetWindowRect(popup_hwnd, &mut cur_r);
        (cur_r.left, cur_r.top)
    };

    if !monitor.is_null() {
        let mut mi: MONITORINFO = zeroed();
        mi.cbSize = size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(monitor, &mut mi) != 0 {
            let margin = scale_px(8, dpi);
            let min_x = mi.rcWork.left + margin;
            let max_x = (mi.rcWork.right - phys_w - margin).max(min_x);
            let min_y = mi.rcWork.top + margin;
            let max_y = (mi.rcWork.bottom - phys_h - margin).max(min_y);
            target_x = target_x.clamp(min_x, max_x);
            target_y = target_y.clamp(min_y, max_y);
        }
    }

    if reposition_at_cursor {
        POPUP_VISIBLE.store(true, Ordering::SeqCst);
        SetWindowPos(
            popup_hwnd,
            HWND_TOPMOST,
            target_x,
            target_y,
            phys_w,
            phys_h,
            SWP_SHOWWINDOW,
        );
        ShowWindow(popup_hwnd, SW_SHOW);
        let _ = SetForegroundWindow(popup_hwnd);
        if mode != ConsoleMode::Settings {
            let _ = SetFocus(edit_hwnd);
        }
    } else {
        SetWindowPos(
            popup_hwnd,
            HWND_TOPMOST,
            target_x,
            target_y,
            phys_w,
            phys_h,
            SWP_NOACTIVATE,
        );
    }

    InvalidateRect(popup_hwnd, null(), 0);
}

unsafe fn set_edit_text_silently(text: &str) {
    let edit_hwnd = EDIT_HWND_ATOMIC.load(Ordering::SeqCst) as HWND;
    if !edit_hwnd.is_null() {
        SUPPRESS_EDIT_CHANGE.store(true, Ordering::SeqCst);
        SetWindowTextW(edit_hwnd, to_wide(text).as_ptr());
        SUPPRESS_EDIT_CHANGE.store(false, Ordering::SeqCst);
    }
}

pub unsafe fn open_popup_in_mode(target_mode: ConsoleMode, toggle_if_same: bool) {
    ensure_ui_initialized();
    clear_recording_target();

    let should_close = with_ui(|st| is_popup_visible() && toggle_if_same && st.mode == target_mode)
        .unwrap_or(false);

    if should_close {
        hide_popup_and_focus_ae();
        return;
    }

    set_edit_text_silently("");

    with_ui(|st| {
        st.mode = target_mode;
        st.toast_error.clear();
        st.query.clear();
        st.selected_pos = 0;
        st.scroll_offset = 0;
        st.hover = HoverTarget::None;
        st.dragging_scrollbar = false;
        if target_mode == ConsoleMode::UsedEffects {
            st.used_scope = if st.settings.default_used_scope == "active" {
                UsedScope::ActiveComp
            } else {
                UsedScope::AllProject
            };
        }
    });

    sync_data_outside_borrow(target_mode, true);

    with_ui(|st| {
        recompute_filtered(st);
    });

    apply_window_layout(true);
}

pub unsafe fn hide_popup_and_focus_ae() {
    clear_recording_target();
    with_ui(|st| st.dragging_scrollbar = false);
    POPUP_VISIBLE.store(false, Ordering::SeqCst);
    let popup = get_popup_hwnd();
    if !popup.is_null() {
        ShowWindow(popup, SW_HIDE);
    }
    let ae_hwnd = get_ae_main_hwnd() as HWND;
    if !ae_hwnd.is_null() && IsWindow(ae_hwnd) != 0 {
        let _ = SetForegroundWindow(ae_hwnd);
    }
}

unsafe fn hide_popup_without_stealing_click() {
    clear_recording_target();
    with_ui(|st| st.dragging_scrollbar = false);
    POPUP_VISIBLE.store(false, Ordering::SeqCst);
    let popup = get_popup_hwnd();
    if !popup.is_null() {
        ShowWindow(popup, SW_HIDE);
    }
}

unsafe fn switch_mode_inline(new_mode: ConsoleMode, query_ae_host: bool) {
    clear_recording_target();
    with_ui(|st| {
        st.mode = new_mode;
        st.toast_error.clear();
        st.selected_pos = 0;
        st.scroll_offset = 0;
        st.dragging_scrollbar = false;
    });

    sync_data_outside_borrow(new_mode, query_ae_host);

    with_ui(|st| {
        recompute_filtered(st);
    });

    apply_window_layout(false);

    let edit_hwnd = EDIT_HWND_ATOMIC.load(Ordering::SeqCst) as HWND;
    let popup_hwnd = get_popup_hwnd();
    if new_mode != ConsoleMode::Settings && !edit_hwnd.is_null() {
        let _ = SetFocus(edit_hwnd);
    } else if !popup_hwnd.is_null() {
        let _ = SetFocus(popup_hwnd);
    }
}

fn localize_error(raw: &str, is_en: bool) -> String {
    if !is_en {
        return raw.to_string();
    }
    if raw.contains("打开并激活一个合成") {
        "Please open and activate a Composition in After Effects first.".to_string()
    } else if raw.contains("不支持添加该效果") {
        "Selected layer does not support adding this effect (e.g. Camera/Light).".to_string()
    } else if raw.contains("找不到预设文件") {
        "Preset file (.ffx) not found on disk.".to_string()
    } else if raw.contains("未找到目标合成") {
        "Target Composition was not found.".to_string()
    } else if raw.contains("不支持启用/禁用") {
        "This effect cannot be toggled on/off.".to_string()
    } else {
        raw.to_string()
    }
}

unsafe fn execute_selected(alt_held: bool) {
    enum PendingExec {
        ApplyEffect {
            id: String,
            name: String,
            match_name: String,
            force_adj: bool,
        },
        ApplyPreset {
            id: String,
            name: String,
            file_path: String,
            force_adj: bool,
        },
        RevealUsed {
            used_idx: usize,
            comp_id: i32,
            comp_name: String,
            layer_index: i32,
            effect_index: i32,
            match_name: String,
            effect_name: String,
            toggle_only: bool,
        },
    }

    let pending = with_ui(|st| -> Option<PendingExec> {
        let &data_idx = st.filtered_indices.get(st.selected_pos)?;
        match st.mode {
            ConsoleMode::ApplyFx => {
                let item = st.catalog.get(data_idx)?;
                match &item.kind {
                    CatalogKind::Effect { match_name } => Some(PendingExec::ApplyEffect {
                        id: item.id.clone(),
                        name: item.name.clone(),
                        match_name: match_name.clone(),
                        force_adj: alt_held,
                    }),
                    CatalogKind::Preset { file_path, .. } => Some(PendingExec::ApplyPreset {
                        id: item.id.clone(),
                        name: item.name.clone(),
                        file_path: file_path.clone(),
                        force_adj: alt_held,
                    }),
                }
            }
            ConsoleMode::UsedEffects => {
                let u = st.used_effects.get(data_idx)?;
                Some(PendingExec::RevealUsed {
                    used_idx: data_idx,
                    comp_id: u.raw.comp_id,
                    comp_name: u.raw.comp_name.clone(),
                    layer_index: u.raw.layer_index,
                    effect_index: u.raw.effect_index,
                    match_name: u.raw.match_name.clone(),
                    effect_name: u.raw.effect_name.clone(),
                    toggle_only: alt_held,
                })
            }
            ConsoleMode::Settings => None,
        }
    })
    .flatten();

    let Some(action) = pending else {
        return;
    };

    match action {
        PendingExec::ApplyEffect {
            id,
            name,
            match_name,
            force_adj,
        } => {
            let res = dispatch_ae_script(
                "applyEffect",
                json!({
                    "matchName": match_name,
                    "displayName": name,
                    "forceAdjustmentLayer": force_adj
                }),
            );
            handle_exec_result(res, Some(id));
        }

        PendingExec::ApplyPreset {
            id,
            name,
            file_path,
            force_adj,
        } => {
            let res = dispatch_ae_script(
                "applyPreset",
                json!({
                    "filePath": file_path,
                    "presetName": name,
                    "forceAdjustmentLayer": force_adj
                }),
            );
            handle_exec_result(res, Some(id));
        }

        PendingExec::RevealUsed {
            used_idx,
            comp_id,
            comp_name,
            layer_index,
            effect_index,
            match_name,
            effect_name,
            toggle_only,
        } => {
            let sub_action = if toggle_only { "toggle" } else { "reveal" };
            let res = dispatch_ae_script(
                "revealUsedEffect",
                json!({
                    "compId": comp_id,
                    "compName": comp_name,
                    "layerIndex": layer_index,
                    "effectIndex": effect_index,
                    "matchName": match_name,
                    "effectName": effect_name,
                    "subAction": sub_action
                }),
            );
            match res {
                Ok(val) if val.get("ok").and_then(|v| v.as_bool()) == Some(true) => {
                    if toggle_only {
                        let new_en = val
                            .get("effectEnabled")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(true);
                        with_ui(|st| {
                            if let Some(u) = st.used_effects.get_mut(used_idx) {
                                u.raw.effect_enabled = new_en;
                            }
                        });
                        InvalidateRect(get_popup_hwnd(), null(), 0);
                    } else {
                        hide_popup_and_focus_ae();
                    }
                }
                Ok(val) => {
                    let msg = val
                        .get("error")
                        .and_then(|v| v.as_str())
                        .unwrap_or("定位失败")
                        .to_string();
                    show_inline_error(&msg);
                }
                Err(err) => show_inline_error(&err),
            }
        }
    }
}

unsafe fn handle_exec_result(res: Result<serde_json::Value, String>, record_id: Option<String>) {
    match res {
        Ok(val) if val.get("ok").and_then(|v| v.as_bool()) == Some(true) => {
            if let Some(id) = record_id {
                with_ui(|st| {
                    st.usage.record_use(&id);
                    save_usage(&st.usage);
                });
            }
            hide_popup_and_focus_ae();
        }
        Ok(val) => {
            let msg = val
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("执行失败")
                .to_string();
            show_inline_error(&msg);
        }
        Err(err) => show_inline_error(&err),
    }
}

unsafe fn show_inline_error(msg: &str) {
    if msg.contains("Esc key held") {
        return;
    }
    with_ui(|st| {
        let is_en = st.settings.language == "en";
        st.toast_error = localize_error(msg, is_en);
    });
    apply_window_layout(false);
}

unsafe fn toggle_star_at_filtered_pos(filtered_pos: usize) {
    with_ui(|st| {
        if st.mode != ConsoleMode::ApplyFx {
            return;
        }
        let Some(&cat_idx) = st.filtered_indices.get(filtered_pos) else {
            return;
        };
        let item_id = st.catalog[cat_idx].id.clone();
        if let Some(p) = st.settings.favorites.iter().position(|x| x == &item_id) {
            st.settings.favorites.remove(p);
            st.catalog[cat_idx].starred = false;
        } else {
            st.settings.favorites.push(item_id);
            st.catalog[cat_idx].starred = true;
        }
        save_settings(&st.settings);
    });
    InvalidateRect(get_popup_hwnd(), null(), 0);
}

unsafe fn fill_solid_rect(hdc: HDC, r: &RECT, col: COLORREF) {
    let br = CreateSolidBrush(col);
    FillRect(hdc, r, br);
    DeleteObject(br as _);
}

unsafe fn draw_round_box(hdc: HDC, r: &RECT, radius: i32, fill_col: COLORREF, border_col: COLORREF) {
    let br = CreateSolidBrush(fill_col);
    let pen = CreatePen(PS_SOLID, 1, border_col);
    let old_br = SelectObject(hdc, br as _);
    let old_pen = SelectObject(hdc, pen as _);
    RoundRect(hdc, r.left, r.top, r.right, r.bottom, radius, radius);
    SelectObject(hdc, old_br);
    SelectObject(hdc, old_pen);
    DeleteObject(br as _);
    DeleteObject(pen as _);
}

unsafe fn draw_text_line(hdc: HDC, text: &str, mut r: RECT, col: COLORREF, flags: u32) {
    SetTextColor(hdc, col);
    let w = to_wide(text);
    DrawTextW(
        hdc,
        w.as_ptr(),
        (w.len() - 1) as i32,
        &mut r,
        flags | DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX | DT_END_ELLIPSIS,
    );
}

enum PaintRow {
    Fx {
        vis_pos: usize,
        is_sel: bool,
        is_hov: bool,
        badge_text: &'static str,
        badge_bg: COLORREF,
        badge_fg: COLORREF,
        title: String,
        category: String,
        starred: bool,
    },
    Used {
        vis_pos: usize,
        is_sel: bool,
        is_hov: bool,
        effect_enabled: bool,
        fx_label: String,
        swatch_col: COLORREF,
        meta_line: String,
        is_active_comp: bool,
    },
}

struct PaintSnapshot {
    dpi: u32,
    is_en: bool,
    recording_target: u32,
    hover: HoverTarget,
    dragging_scrollbar: bool,
    mode: ConsoleMode,
    fx_filter_label: &'static str,
    used_scope: UsedScope,
    used_total_count: usize,
    total_filtered: usize,
    scroll_offset: usize,
    ctx_text: String,
    toast_error: String,
    hotkey_fx: String,
    hotkey_used: String,
    default_scope_str: String,
    rows: Vec<PaintRow>,
}

fn has_cjk(s: &str) -> bool {
    s.chars().any(|c| ('\u{4E00}'..='\u{9FFF}').contains(&c))
}

fn build_paint_snapshot(st: &UiState) -> PaintSnapshot {
    let dpi = CURRENT_DPI_ATOMIC.load(Ordering::SeqCst).max(96);
    let is_en = st.settings.language == "en";
    let recording_target = get_recording_target();

    let ctx_text = if st.mode == ConsoleMode::ApplyFx {
        if st.active_comp_name.is_empty() {
            if is_en {
                format!("{} items", st.catalog.len())
            } else {
                format!("共 {} 项", st.catalog.len())
            }
        } else if st.selected_layer_count > 0 {
            if is_en {
                format!("{} selected", st.selected_layer_count)
            } else {
                format!("已选 {} 层", st.selected_layer_count)
            }
        } else if is_en {
            "No layer (Auto Adj)".to_string()
        } else {
            "未选层(自动新建调整层)".to_string()
        }
    } else if is_en {
        format!("{} matched", st.filtered_indices.len())
    } else {
        format!("匹配 {} 项", st.filtered_indices.len())
    };

    let mut rows = Vec::new();
    if st.mode != ConsoleMode::Settings && !st.filtered_indices.is_empty() {
        let end = (st.scroll_offset + MAX_VISIBLE_ROWS).min(st.filtered_indices.len());
        for vis_pos in st.scroll_offset..end {
            let data_idx = st.filtered_indices[vis_pos];
            let is_sel = vis_pos == st.selected_pos;
            let is_hov = st.hover == HoverTarget::Row(vis_pos);

            if st.mode == ConsoleMode::ApplyFx {
                let item = &st.catalog[data_idx];
                let (badge_text, badge_bg, badge_fg) = match &item.kind {
                    CatalogKind::Effect { .. } => ("FX", rgb(18, 48, 70), COL_CYAN),
                    CatalogKind::Preset { source, .. } if source == "user" || source == "custom" => {
                        (
                            if is_en { "USER" } else { "用户" },
                            rgb(62, 44, 14),
                            COL_AMBER,
                        )
                    }
                    CatalogKind::Preset { .. } => ("FFX", rgb(16, 52, 42), COL_GREEN),
                };

                let title = if item.secondary_name.is_empty() {
                    item.name.clone()
                } else if is_en {
                    if !has_cjk(&item.name) {
                        item.name.clone()
                    } else {
                        format!("{}   {}", item.secondary_name, item.name)
                    }
                } else if has_cjk(&item.name) {
                    format!("{}   {}", item.name, item.secondary_name)
                } else {
                    format!("{}   {}", item.secondary_name, item.name)
                };

                rows.push(PaintRow::Fx {
                    vis_pos,
                    is_sel,
                    is_hov,
                    badge_text,
                    badge_bg,
                    badge_fg,
                    title,
                    category: item.category.clone(),
                    starred: item.starred,
                });
            } else {
                let u = &st.used_effects[data_idx].raw;
                let fx_label = if u.effect_enabled {
                    u.effect_name.clone()
                } else if is_en {
                    format!("{} (fx off)", u.effect_name)
                } else {
                    format!("{} (已关闭 fx)", u.effect_name)
                };
                let swatch_col = AE_LABEL_COLORS[(u.layer_label.clamp(0, 16)) as usize];
                let meta_line = if u.is_active_comp {
                    if is_en {
                        format!("[Active · {}]  ›  #{} {}", u.comp_name, u.layer_index, u.layer_name)
                    } else {
                        format!("[当前 · {}]  ›  #{} {}", u.comp_name, u.layer_index, u.layer_name)
                    }
                } else if is_en {
                    format!("[Comp · {}]  ›  #{} {}", u.comp_name, u.layer_index, u.layer_name)
                } else {
                    format!("[合成 · {}]  ›  #{} {}", u.comp_name, u.layer_index, u.layer_name)
                };
                rows.push(PaintRow::Used {
                    vis_pos,
                    is_sel,
                    is_hov,
                    effect_enabled: u.effect_enabled,
                    fx_label,
                    swatch_col,
                    meta_line,
                    is_active_comp: u.is_active_comp,
                });
            }
        }
    }

    PaintSnapshot {
        dpi,
        is_en,
        recording_target,
        hover: st.hover,
        dragging_scrollbar: st.dragging_scrollbar,
        mode: st.mode,
        fx_filter_label: st.fx_filter.label(is_en),
        used_scope: st.used_scope,
        used_total_count: st.used_effects.len(),
        total_filtered: st.filtered_indices.len(),
        scroll_offset: st.scroll_offset,
        ctx_text,
        toast_error: st.toast_error.clone(),
        hotkey_fx: st.settings.hotkey_fx.clone(),
        hotkey_used: st.settings.hotkey_used.clone(),
        default_scope_str: st.settings.default_used_scope.clone(),
        rows,
    }
}

unsafe fn paint_popup(hwnd: HWND, hdc_dest: HDC) {
    let Some(snap) = with_ui(|st| build_paint_snapshot(st)) else {
        return;
    };

    let mut client: RECT = zeroed();
    GetClientRect(hwnd, &mut client);
    let width = (client.right - client.left).max(1);
    let height = (client.bottom - client.top).max(1);

    let mem_dc = CreateCompatibleDC(hdc_dest);
    let mut bmi: BITMAPINFO = zeroed();
    bmi.bmiHeader = BITMAPINFOHEADER {
        biSize: size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: width,
        biHeight: -height,
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB,
        ..zeroed()
    };
    let mut bits_void: *mut c_void = null_mut();
    let hbm = CreateDIBSection(mem_dc, &bmi, DIB_RGB_COLORS, &mut bits_void, null_mut(), 0);
    let bits = bits_void as *mut u32;
    let old_bm = SelectObject(mem_dc, hbm as _);
    SetBkMode(mem_dc, TRANSPARENT as i32);

    let mut new_hits: Vec<HitBox> = Vec::with_capacity(32);
    let s = |v: i32| scale_px(v, snap.dpi);

    // Defer vector SDF icon draw commands until GdiFlush() so GDI + DIB pixel blends are 100% ordered
    struct PendingIcon {
        cx: i32,
        cy: i32,
        size: i32,
        col: COLORREF,
        sdf: fn(f32, f32) -> f32,
    }
    let mut deferred_icons: Vec<PendingIcon> = Vec::with_capacity(3);

    let mut computed_sb_top = 0i32;
    let mut computed_sb_h = 1i32;
    let mut computed_thumb_h = 1i32;

    let f_bold = create_ui_font(s(13), 650);
    let f_item = create_ui_font(s(14), 500);
    let f_small = create_ui_font(s(11), 400);
    let f_badge = create_ui_font(s(10), 700);

    let outer_border = if snap.mode == ConsoleMode::UsedEffects {
        COL_BORDER_USED
    } else {
        COL_BORDER_FX
    };
    draw_round_box(mem_dc, &client, s(12), COL_BG_SHELL, outer_border);

    // 1. Top Header Bar
    let top_h = s(38);
    fill_solid_rect(
        mem_dc,
        &RECT {
            left: 1,
            top: 1,
            right: width - 1,
            bottom: top_h,
        },
        COL_BG_TOPBAR,
    );
    fill_solid_rect(
        mem_dc,
        &RECT {
            left: 1,
            top: top_h,
            right: width - 1,
            bottom: top_h + 1,
        },
        COL_BORDER_SUBTLE,
    );

    let old_f = SelectObject(mem_dc, f_bold as _);

    // Tab 1: ⚡ FXSearch · 添加 (with hover highlight)
    let tab1_text = if snap.is_en {
        "⚡ FXSearch · Add"
    } else {
        "⚡ FXSearch · 添加"
    };
    let tab1_w = measure_text_width(mem_dc, tab1_text) + s(26);
    let tab1_r = RECT {
        left: s(10),
        top: s(6),
        right: s(10) + tab1_w,
        bottom: s(32),
    };
    let tab1_hov = snap.hover == HoverTarget::TabFx;
    if snap.mode == ConsoleMode::ApplyFx {
        draw_round_box(
            mem_dc,
            &tab1_r,
            s(6),
            if tab1_hov { COL_BG_SEL_FX_HOV } else { COL_BG_SEL_FX },
            if tab1_hov { COL_CYAN } else { COL_BORDER_FX },
        );
        draw_text_line(mem_dc, tab1_text, tab1_r, COL_CYAN, DT_CENTER);
    } else if tab1_hov {
        draw_round_box(
            mem_dc,
            &tab1_r,
            s(6),
            COL_BG_TOPBTN_HOVER,
            COL_BORDER_TOPBTN_HOVER,
        );
        draw_text_line(mem_dc, tab1_text, tab1_r, COL_TEXT_MAIN, DT_CENTER);
    } else {
        draw_text_line(mem_dc, tab1_text, tab1_r, COL_TEXT_MUTED, DT_CENTER);
    }
    new_hits.push(HitBox {
        rect: tab1_r,
        action: HitAction::TabFx,
    });

    // Tab 2: [boxicons--target icon] + 查找已用效果 (with hover highlight)
    let used_label_only = if snap.is_en {
        if snap.used_total_count > 0 {
            format!("Used Effects ({})", snap.used_total_count)
        } else {
            "Used Effects".to_string()
        }
    } else if snap.used_total_count > 0 {
        format!("查找已用效果 ({})", snap.used_total_count)
    } else {
        "查找已用效果".to_string()
    };
    let icon_sz = s(15);
    let icon_gap = s(6);
    let tab2_text_w = measure_text_width(mem_dc, &used_label_only);
    let tab2_left = tab1_r.right + s(8);
    let tab2_w = s(12) + icon_sz + icon_gap + tab2_text_w + s(13);
    let tab2_r = RECT {
        left: tab2_left,
        top: s(6),
        right: tab2_left + tab2_w,
        bottom: s(32),
    };
    let tab2_hov = snap.hover == HoverTarget::TabUsed;
    let tab2_fg = if snap.mode == ConsoleMode::UsedEffects {
        draw_round_box(
            mem_dc,
            &tab2_r,
            s(6),
            if tab2_hov {
                COL_BG_SEL_USED_HOV
            } else {
                COL_BG_SEL_USED
            },
            if tab2_hov { COL_VIOLET } else { COL_BORDER_USED },
        );
        COL_VIOLET
    } else if tab2_hov {
        draw_round_box(
            mem_dc,
            &tab2_r,
            s(6),
            COL_BG_TOPBTN_HOVER,
            COL_BORDER_TOPBTN_HOVER,
        );
        COL_TEXT_MAIN
    } else {
        COL_TEXT_MUTED
    };

    deferred_icons.push(PendingIcon {
        cx: tab2_r.left + s(12) + icon_sz / 2,
        cy: (tab2_r.top + tab2_r.bottom) / 2,
        size: icon_sz,
        col: tab2_fg,
        sdf: sdf_boxicons_target,
    });
    draw_text_line(
        mem_dc,
        &used_label_only,
        RECT {
            left: tab2_r.left + s(12) + icon_sz + icon_gap,
            top: tab2_r.top,
            right: tab2_r.right - s(10),
            bottom: tab2_r.bottom,
        },
        tab2_fg,
        DT_LEFT,
    );
    new_hits.push(HitBox {
        rect: tab2_r,
        action: HitAction::TabUsed,
    });

    // Settings Button on top-right (`ant-design--setting-outlined` vector icon + hover highlight)
    let gear_r = RECT {
        left: width - s(34),
        top: s(6),
        right: width - s(8),
        bottom: s(32),
    };
    let gear_hov = snap.hover == HoverTarget::SettingsBtn;
    let gear_fg = if snap.mode == ConsoleMode::Settings {
        draw_round_box(
            mem_dc,
            &gear_r,
            s(6),
            if gear_hov { COL_BG_SEL_FX_HOV } else { COL_BG_SEL_FX },
            if gear_hov { COL_CYAN } else { COL_BORDER_FX },
        );
        COL_CYAN
    } else if gear_hov {
        draw_round_box(
            mem_dc,
            &gear_r,
            s(6),
            COL_BG_TOPBTN_HOVER,
            COL_BORDER_TOPBTN_HOVER,
        );
        COL_TEXT_MAIN
    } else {
        draw_round_box(mem_dc, &gear_r, s(6), rgb(24, 27, 36), COL_BORDER_SUBTLE);
        COL_TEXT_MUTED
    };
    deferred_icons.push(PendingIcon {
        cx: (gear_r.left + gear_r.right) / 2,
        cy: (gear_r.top + gear_r.bottom) / 2,
        size: s(16),
        col: gear_fg,
        sdf: sdf_ant_setting_outlined,
    });
    new_hits.push(HitBox {
        rect: gear_r,
        action: HitAction::ToggleSettings,
    });

    // Right Pill: Neutral colorless style for BOTH FX Type Filter AND Used Effects Scope (without "(Alt+C)") + Hover highlight!
    SelectObject(mem_dc, f_small as _);
    let mut right_pill_left = gear_r.left - s(6);
    let pill_label_opt: Option<&str> = if snap.mode == ConsoleMode::ApplyFx {
        Some(snap.fx_filter_label)
    } else if snap.mode == ConsoleMode::UsedEffects {
        Some(match (snap.used_scope, snap.is_en) {
            (UsedScope::AllProject, false) => "范围: 全工程合成",
            (UsedScope::ActiveComp, false) => "范围: 当前合成",
            (UsedScope::AllProject, true) => "Scope: All Comps",
            (UsedScope::ActiveComp, true) => "Scope: Active Comp",
        })
    } else {
        None
    };

    if let Some(pill_txt) = pill_label_opt {
        let pill_w = measure_text_width(mem_dc, pill_txt) + s(24);
        right_pill_left = gear_r.left - s(6) - pill_w;
        let pill_r = RECT {
            left: right_pill_left,
            top: s(6),
            right: gear_r.left - s(6),
            bottom: s(32),
        };
        let pill_hov = snap.hover == HoverTarget::FilterOrScope;
        draw_round_box(
            mem_dc,
            &pill_r,
            s(6),
            if pill_hov {
                COL_BG_TOPBTN_HOVER
            } else {
                rgb(26, 30, 40)
            },
            if pill_hov {
                COL_BORDER_TOPBTN_HOVER
            } else {
                COL_BORDER_SUBTLE
            },
        );
        draw_text_line(
            mem_dc,
            pill_txt,
            pill_r,
            if pill_hov { COL_TEXT_MAIN } else { COL_TEXT_MUTED },
            DT_CENTER,
        );
        new_hits.push(HitBox {
            rect: pill_r,
            action: HitAction::ToggleFilterOrScope,
        });
    }

    if right_pill_left > tab2_r.right + s(6) {
        new_hits.push(HitBox {
            rect: RECT {
                left: tab2_r.right + s(4),
                top: 0,
                right: right_pill_left - s(4),
                bottom: top_h,
            },
            action: HitAction::DragHeader,
        });
    }

    let mut cur_y = top_h + 1;

    if snap.mode == ConsoleMode::Settings {
        let pad_x = s(16);
        let row_h = s(36);
        cur_y += s(10);

        let fx_key_display = if snap.recording_target == 1 {
            if snap.is_en {
                "Press shortcut keys... (Esc cancel)".to_string()
            } else {
                "请按下组合快捷键... (Esc 取消)".to_string()
            }
        } else {
            snap.hotkey_fx
        };

        let used_key_display = if snap.recording_target == 2 {
            if snap.is_en {
                "Press shortcut keys... (Esc cancel)".to_string()
            } else {
                "请按下组合快捷键... (Esc 取消)".to_string()
            }
        } else {
            snap.hotkey_used
        };

        let lang_val = if snap.is_en {
            "English (Click to switch 中文)".to_string()
        } else {
            "简体中文 (点击切换 English)".to_string()
        };

        let scope_val = match (snap.default_scope_str.as_str(), snap.is_en) {
            ("active", false) => "仅当前激活合成".to_string(),
            (_, false) => "全工程所有合成 (推荐)".to_string(),
            ("active", true) => "Active Composition Only".to_string(),
            (_, true) => "All Comps in Project (Default)".to_string(),
        };

        let settings_rows: [(&str, String, HitAction, bool, usize); 4] = [
            (
                if snap.is_en {
                    "Interface Language / 界面语言"
                } else {
                    "界面语言 / Language"
                },
                lang_val,
                HitAction::ToggleLanguage,
                false,
                0,
            ),
            (
                if snap.is_en {
                    "Open FXSearch Shortcut (Click & Press Keys)"
                } else {
                    "「添加效果 / 预设」呼出热键（点击后直接按键）"
                },
                fx_key_display,
                HitAction::StartRecordFxHotkey,
                snap.recording_target == 1,
                1,
            ),
            (
                if snap.is_en {
                    "Locate Used Effects Shortcut (Click & Press Keys)"
                } else {
                    "「查找已用效果」直达热键（点击后直接按键）"
                },
                used_key_display,
                HitAction::StartRecordUsedHotkey,
                snap.recording_target == 2,
                2,
            ),
            (
                if snap.is_en {
                    "Default Used Effects Search Scope"
                } else {
                    "「查找已用效果」默认扫描范围（点击切换）"
                },
                scope_val,
                HitAction::ToggleDefaultScope,
                false,
                3,
            ),
        ];

        for (label, val_str, act, is_rec, s_idx) in settings_rows {
            let r = RECT {
                left: pad_x,
                top: cur_y,
                right: width - pad_x,
                bottom: cur_y + s(32),
            };
            let is_s_hov = snap.hover == HoverTarget::SettingsItem(s_idx);
            draw_round_box(
                mem_dc,
                &r,
                s(6),
                if is_rec {
                    COL_BG_RECORDING
                } else if is_s_hov {
                    COL_BG_TOPBTN_HOVER
                } else {
                    COL_BG_SEARCH
                },
                if is_rec {
                    COL_AMBER
                } else if is_s_hov {
                    COL_BORDER_TOPBTN_HOVER
                } else {
                    COL_BORDER_SUBTLE
                },
            );
            SelectObject(mem_dc, f_item as _);
            draw_text_line(
                mem_dc,
                label,
                RECT {
                    left: r.left + s(12),
                    right: r.right - s(210),
                    ..r
                },
                COL_TEXT_MAIN,
                DT_LEFT,
            );
            SelectObject(mem_dc, f_bold as _);
            draw_text_line(
                mem_dc,
                &val_str,
                RECT {
                    left: r.right - s(215),
                    right: r.right - s(12),
                    ..r
                },
                if is_rec { COL_AMBER } else { COL_CYAN },
                DT_RIGHT,
            );
            new_hits.push(HitBox {
                rect: r,
                action: act,
            });
            cur_y += row_h;
        }

        cur_y += s(8);
        let total_inner_w = width - pad_x * 2;
        let gap = s(10);
        let col_w = (total_inner_w - gap * 2) / 3;

        let btn_rescan = RECT {
            left: pad_x,
            top: cur_y,
            right: pad_x + col_w,
            bottom: cur_y + s(32),
        };
        let hov_rescan = snap.hover == HoverTarget::SettingsItem(4);
        draw_round_box(
            mem_dc,
            &btn_rescan,
            s(6),
            if hov_rescan {
                COL_BG_SEL_FX_HOV
            } else {
                COL_BG_SEL_FX
            },
            if hov_rescan { COL_CYAN } else { COL_BORDER_FX },
        );
        SelectObject(mem_dc, f_bold as _);
        draw_text_line(
            mem_dc,
            if snap.is_en {
                "Refresh List"
            } else {
                "手动刷新列表"
            },
            btn_rescan,
            COL_CYAN,
            DT_CENTER,
        );
        new_hits.push(HitBox {
            rect: btn_rescan,
            action: HitAction::RescanCatalog,
        });

        let btn_dir = RECT {
            left: btn_rescan.right + gap,
            top: cur_y,
            right: btn_rescan.right + gap + col_w,
            bottom: cur_y + s(32),
        };
        let hov_dir = snap.hover == HoverTarget::SettingsItem(5);
        draw_round_box(
            mem_dc,
            &btn_dir,
            s(6),
            if hov_dir {
                COL_BG_TOPBTN_HOVER
            } else {
                COL_BG_SEARCH
            },
            if hov_dir {
                COL_BORDER_TOPBTN_HOVER
            } else {
                COL_BORDER_SUBTLE
            },
        );
        draw_text_line(
            mem_dc,
            if snap.is_en {
                "Open Config Dir"
            } else {
                "打开配置文件夹"
            },
            btn_dir,
            COL_TEXT_MAIN,
            DT_CENTER,
        );
        new_hits.push(HitBox {
            rect: btn_dir,
            action: HitAction::OpenConfigFolder,
        });

        let btn_change_dir = RECT {
            left: btn_dir.right + gap,
            top: cur_y,
            right: width - pad_x,
            bottom: cur_y + s(32),
        };
        let hov_chg = snap.hover == HoverTarget::SettingsItem(6);
        draw_round_box(
            mem_dc,
            &btn_change_dir,
            s(6),
            if hov_chg {
                COL_BG_TOPBTN_HOVER
            } else {
                COL_BG_SEARCH
            },
            if hov_chg {
                COL_BORDER_TOPBTN_HOVER
            } else {
                COL_BORDER_SUBTLE
            },
        );
        draw_text_line(
            mem_dc,
            if snap.is_en {
                "Set Config Dir..."
            } else {
                "更改配置文件夹..."
            },
            btn_change_dir,
            COL_TEXT_MAIN,
            DT_CENTER,
        );
        new_hits.push(HitBox {
            rect: btn_change_dir,
            action: HitAction::ChangeConfigFolder,
        });
    } else {
        // 2. Search Bar
        let search_h = s(46);
        fill_solid_rect(
            mem_dc,
            &RECT {
                left: 1,
                top: cur_y,
                right: width - 1,
                bottom: cur_y + search_h,
            },
            COL_BG_SEARCH,
        );
        fill_solid_rect(
            mem_dc,
            &RECT {
                left: 1,
                top: cur_y + search_h,
                right: width - 1,
                bottom: cur_y + search_h + 1,
            },
            COL_BORDER_SUBTLE,
        );

        let prefix_r = RECT {
            left: s(12),
            top: cur_y + s(11),
            right: s(48),
            bottom: cur_y + s(35),
        };
        SelectObject(mem_dc, f_badge as _);
        if snap.mode == ConsoleMode::ApplyFx {
            draw_round_box(mem_dc, &prefix_r, s(5), rgb(2, 132, 199), COL_CYAN);
            draw_text_line(mem_dc, "FX", prefix_r, COL_TEXT_MAIN, DT_CENTER);
        } else {
            // Left search badge in Used Effects mode: crisp `boxicons--target` vector icon!
            draw_round_box(mem_dc, &prefix_r, s(5), rgb(124, 58, 237), COL_VIOLET);
            deferred_icons.push(PendingIcon {
                cx: (prefix_r.left + prefix_r.right) / 2,
                cy: (prefix_r.top + prefix_r.bottom) / 2,
                size: s(16),
                col: COL_TEXT_MAIN,
                sdf: sdf_boxicons_target,
            });
        }

        SelectObject(mem_dc, f_small as _);
        draw_text_line(
            mem_dc,
            &snap.ctx_text,
            RECT {
                left: width - s(154),
                top: cur_y,
                right: width - s(12),
                bottom: cur_y + search_h,
            },
            COL_TEXT_DIM,
            DT_RIGHT,
        );

        cur_y += search_h + 1;

        if !snap.toast_error.is_empty() {
            let err_h = s(28);
            let err_r = RECT {
                left: 1,
                top: cur_y,
                right: width - 1,
                bottom: cur_y + err_h,
            };
            fill_solid_rect(mem_dc, &err_r, COL_RED_BG);
            draw_text_line(
                mem_dc,
                &snap.toast_error,
                RECT {
                    left: s(12),
                    right: width - s(12),
                    ..err_r
                },
                COL_RED_TEXT,
                DT_LEFT,
            );
            cur_y += err_h;
        }

        let list_top_y = cur_y + s(5);
        cur_y = list_top_y;

        let has_scrollbar = snap.total_filtered > MAX_VISIBLE_ROWS;
        let row_right_edge = if has_scrollbar {
            width - s(15)
        } else {
            width - s(6)
        };

        if snap.rows.is_empty() {
            SelectObject(mem_dc, f_item as _);
            let empty_msg = match (snap.mode, snap.used_scope, snap.is_en) {
                (ConsoleMode::UsedEffects, UsedScope::ActiveComp, false) => {
                    "当前合成暂无匹配的已使用效果（按 Alt+C 可切换扫描全工程合成）"
                }
                (ConsoleMode::UsedEffects, UsedScope::AllProject, false) => {
                    "当前工程各合成中暂未找到匹配的已使用效果"
                }
                (ConsoleMode::UsedEffects, UsedScope::ActiveComp, true) => {
                    "No matching used effects in Active Comp (Press Alt+C for All Comps)"
                }
                (ConsoleMode::UsedEffects, UsedScope::AllProject, true) => {
                    "No matching used effects found across Project Compositions"
                }
                (_, _, false) => "未找到匹配的效果或 .ffx 动画预设",
                (_, _, true) => "No matching effects or .ffx presets found",
            };
            draw_text_line(
                mem_dc,
                empty_msg,
                RECT {
                    left: s(16),
                    top: cur_y,
                    right: width - s(16),
                    bottom: cur_y + s(56),
                },
                COL_TEXT_MUTED,
                DT_CENTER,
            );
        } else {
            let row_h = if snap.mode == ConsoleMode::ApplyFx {
                s(38)
            } else {
                s(46)
            };

            for row in &snap.rows {
                match row {
                    PaintRow::Fx {
                        vis_pos,
                        is_sel,
                        is_hov,
                        badge_text,
                        badge_bg,
                        badge_fg,
                        title,
                        category,
                        starred,
                    } => {
                        let row_r = RECT {
                            left: s(6),
                            top: cur_y,
                            right: row_right_edge,
                            bottom: cur_y + row_h - s(2),
                        };
                        if *is_sel {
                            draw_round_box(mem_dc, &row_r, s(7), COL_BG_SEL_FX, COL_BORDER_FX);
                        } else if *is_hov {
                            draw_round_box(mem_dc, &row_r, s(7), COL_BG_HOVER, COL_BG_HOVER);
                        }

                        if *vis_pos < 9 {
                            SelectObject(mem_dc, f_small as _);
                            draw_text_line(
                                mem_dc,
                                &format!("{}", vis_pos + 1),
                                RECT {
                                    left: row_r.left + s(6),
                                    top: row_r.top,
                                    right: row_r.left + s(24),
                                    bottom: row_r.bottom,
                                },
                                if *is_sel { COL_CYAN } else { COL_TEXT_DIM },
                                DT_CENTER,
                            );
                        }

                        let badge_r = RECT {
                            left: row_r.left + s(28),
                            top: row_r.top + s(9),
                            right: row_r.left + s(72),
                            bottom: row_r.bottom - s(9),
                        };
                        SelectObject(mem_dc, f_badge as _);
                        draw_round_box(mem_dc, &badge_r, s(4), *badge_bg, *badge_fg);
                        draw_text_line(mem_dc, badge_text, badge_r, *badge_fg, DT_CENTER);

                        SelectObject(mem_dc, f_item as _);
                        draw_text_line(
                            mem_dc,
                            title,
                            RECT {
                                left: row_r.left + s(80),
                                top: row_r.top,
                                right: row_r.right - s(142),
                                bottom: row_r.bottom,
                            },
                            COL_TEXT_MAIN,
                            DT_LEFT,
                        );

                        SelectObject(mem_dc, f_small as _);
                        draw_text_line(
                            mem_dc,
                            category,
                            RECT {
                                left: row_r.right - s(140),
                                top: row_r.top,
                                right: row_r.right - s(32),
                                bottom: row_r.bottom,
                            },
                            COL_TEXT_DIM,
                            DT_RIGHT,
                        );

                        let star_r = RECT {
                            left: row_r.right - s(28),
                            top: row_r.top + s(4),
                            right: row_r.right - s(4),
                            bottom: row_r.bottom - s(4),
                        };
                        SelectObject(mem_dc, f_item as _);
                        draw_text_line(
                            mem_dc,
                            if *starred { "★" } else { "☆" },
                            star_r,
                            if *starred { COL_AMBER } else { COL_TEXT_DIM },
                            DT_CENTER,
                        );
                        new_hits.push(HitBox {
                            rect: row_r,
                            action: HitAction::RowClick(*vis_pos),
                        });
                        new_hits.push(HitBox {
                            rect: star_r,
                            action: HitAction::StarClick(*vis_pos),
                        });
                    }

                    PaintRow::Used {
                        vis_pos,
                        is_sel,
                        is_hov,
                        effect_enabled,
                        fx_label,
                        swatch_col,
                        meta_line,
                        is_active_comp,
                    } => {
                        let row_r = RECT {
                            left: s(6),
                            top: cur_y,
                            right: row_right_edge,
                            bottom: cur_y + row_h - s(2),
                        };
                        if *is_sel {
                            draw_round_box(mem_dc, &row_r, s(7), COL_BG_SEL_USED, COL_BORDER_USED);
                        } else if *is_hov {
                            draw_round_box(mem_dc, &row_r, s(7), COL_BG_HOVER, COL_BG_HOVER);
                        }

                        if *vis_pos < 9 {
                            SelectObject(mem_dc, f_small as _);
                            draw_text_line(
                                mem_dc,
                                &format!("{}", vis_pos + 1),
                                RECT {
                                    left: row_r.left + s(6),
                                    top: row_r.top,
                                    right: row_r.left + s(24),
                                    bottom: row_r.bottom,
                                },
                                if *is_sel { COL_VIOLET } else { COL_TEXT_DIM },
                                DT_CENTER,
                            );
                        }

                        let dot_r = RECT {
                            left: row_r.left + s(30),
                            top: row_r.top + s(10),
                            right: row_r.left + s(38),
                            bottom: row_r.top + s(18),
                        };
                        let dot_col = if *effect_enabled {
                            COL_GREEN
                        } else {
                            COL_TEXT_DIM
                        };
                        draw_round_box(mem_dc, &dot_r, s(4), dot_col, dot_col);

                        SelectObject(mem_dc, f_item as _);
                        draw_text_line(
                            mem_dc,
                            fx_label,
                            RECT {
                                left: row_r.left + s(46),
                                top: row_r.top + s(3),
                                right: row_r.right - s(80),
                                bottom: row_r.top + s(23),
                            },
                            if *effect_enabled {
                                COL_TEXT_MAIN
                            } else {
                                COL_TEXT_MUTED
                            },
                            DT_LEFT,
                        );

                        let swatch_r = RECT {
                            left: row_r.left + s(46),
                            top: row_r.top + s(27),
                            right: row_r.left + s(54),
                            bottom: row_r.top + s(35),
                        };
                        fill_solid_rect(mem_dc, &swatch_r, *swatch_col);

                        SelectObject(mem_dc, f_small as _);
                        draw_text_line(
                            mem_dc,
                            meta_line,
                            RECT {
                                left: row_r.left + s(60),
                                top: row_r.top + s(22),
                                right: row_r.right - s(80),
                                bottom: row_r.bottom - s(2),
                            },
                            if *is_active_comp {
                                COL_CYAN
                            } else {
                                COL_TEXT_MUTED
                            },
                            DT_LEFT,
                        );

                        draw_text_line(
                            mem_dc,
                            if snap.is_en { "Locate ↵" } else { "回车定位 ↵" },
                            RECT {
                                left: row_r.right - s(76),
                                top: row_r.top,
                                right: row_r.right - s(10),
                                bottom: row_r.bottom,
                            },
                            COL_TEXT_DIM,
                            DT_RIGHT,
                        );

                        new_hits.push(HitBox {
                            rect: row_r,
                            action: HitAction::RowClick(*vis_pos),
                        });
                    }
                }
                cur_y += row_h;
            }

            // 3. Draggable Right Vertical Scrollbar
            if has_scrollbar {
                let track_top = list_top_y;
                let track_bottom = cur_y - s(2);
                let track_h = (track_bottom - track_top).max(s(40));

                let track_r = RECT {
                    left: width - s(11),
                    top: track_top,
                    right: width - s(4),
                    bottom: track_bottom,
                };
                draw_round_box(mem_dc, &track_r, s(4), COL_SCROLL_TRACK, COL_SCROLL_TRACK);

                let max_scroll = snap.total_filtered.saturating_sub(MAX_VISIBLE_ROWS).max(1);
                let thumb_h = ((track_h as usize * MAX_VISIBLE_ROWS) / snap.total_filtered)
                    .max(s(26) as usize)
                    .min(track_h as usize) as i32;
                let avail_travel = (track_h - thumb_h).max(0);
                let clamped_off = snap.scroll_offset.min(max_scroll);
                let thumb_top =
                    track_top + ((avail_travel as usize * clamped_off) / max_scroll) as i32;

                let thumb_r = RECT {
                    left: width - s(11),
                    top: thumb_top,
                    right: width - s(4),
                    bottom: thumb_top + thumb_h,
                };
                let sb_active =
                    snap.dragging_scrollbar || snap.hover == HoverTarget::ScrollbarThumb;
                let thumb_col = if sb_active {
                    COL_SCROLL_THUMB_ACTIVE
                } else {
                    COL_SCROLL_THUMB
                };
                draw_round_box(mem_dc, &thumb_r, s(4), thumb_col, thumb_col);

                computed_sb_top = track_top;
                computed_sb_h = track_h;
                computed_thumb_h = thumb_h;

                // Track hit box first, Thumb hit box after (since hit-test scans in reverse)
                new_hits.push(HitBox {
                    rect: RECT {
                        left: width - s(14),
                        top: track_top,
                        right: width - s(2),
                        bottom: track_bottom,
                    },
                    action: HitAction::ScrollbarTrack,
                });
                new_hits.push(HitBox {
                    rect: RECT {
                        left: width - s(14),
                        top: thumb_top,
                        right: width - s(2),
                        bottom: thumb_top + thumb_h,
                    },
                    action: HitAction::ScrollbarThumb,
                });
            }
        }
    }

    // 4. Footer Bar
    let footer_h = s(28);
    let footer_r = RECT {
        left: 1,
        top: height - footer_h - 1,
        right: width - 1,
        bottom: height - 1,
    };
    fill_solid_rect(mem_dc, &footer_r, COL_BG_FOOTER);
    fill_solid_rect(
        mem_dc,
        &RECT {
            left: 1,
            top: footer_r.top - 1,
            right: width - 1,
            bottom: footer_r.top,
        },
        COL_BORDER_SUBTLE,
    );

    SelectObject(mem_dc, f_small as _);
    let footer_str = match (snap.mode, snap.is_en) {
        (ConsoleMode::ApplyFx, false) => {
            "↑↓ 选择  ·  Enter 应用  ·  Alt+Enter 新建调整层  ·  Ctrl+S 收藏  ·  Tab 查找已用效果  ·  Esc 关闭"
        }
        (ConsoleMode::ApplyFx, true) => {
            "↑↓ Select  ·  Enter Apply  ·  Alt+Enter Adj Layer  ·  Ctrl+S Star  ·  Tab Used FX  ·  Esc Close"
        }
        (ConsoleMode::UsedEffects, false) => {
            "↑↓ 选择  ·  Enter 跳转合成并定位图层  ·  Alt+C 切全工程/当前合成  ·  Alt+Enter 开关fx  ·  Tab 返回"
        }
        (ConsoleMode::UsedEffects, true) => {
            "↑↓ Select  ·  Enter Open Comp & Locate  ·  Alt+C All/Active Comp  ·  Alt+Enter Toggle fx  ·  Tab Back"
        }
        (ConsoleMode::Settings, false) => {
            "点击热键栏后直接按下任意组合键即可自定义绑定   ·   按 Esc 返回搜索界面"
        }
        (ConsoleMode::Settings, true) => {
            "Click a shortcut row and press any key combination to bind   ·   Press Esc to return"
        }
    };
    draw_text_line(
        mem_dc,
        footer_str,
        RECT {
            left: s(12),
            right: width - s(12),
            ..footer_r
        },
        COL_TEXT_DIM,
        DT_LEFT,
    );

    SelectObject(mem_dc, old_f);
    DeleteObject(f_bold as _);
    DeleteObject(f_item as _);
    DeleteObject(f_small as _);
    DeleteObject(f_badge as _);

    // Flush GDI pipeline before writing supersampled anti-aliased vector icons into DIB memory
    GdiFlush();
    for ic in deferred_icons {
        draw_sdf_icon(bits, width, height, ic.cx, ic.cy, ic.size, ic.col, ic.sdf);
    }

    BitBlt(hdc_dest, 0, 0, width, height, mem_dc, 0, 0, SRCCOPY);
    SelectObject(mem_dc, old_bm);
    DeleteObject(hbm as _);
    DeleteDC(mem_dc);

    with_ui(|st| {
        st.hit_boxes = new_hits;
        st.sb_track_top = computed_sb_top;
        st.sb_track_h = computed_sb_h;
        st.sb_thumb_h = computed_thumb_h;
    });
}

unsafe fn toggle_filter_or_scope_inline() {
    with_ui(|st| {
        if st.mode == ConsoleMode::ApplyFx {
            st.fx_filter = st.fx_filter.next();
        } else if st.mode == ConsoleMode::UsedEffects {
            st.used_scope = st.used_scope.toggle();
        }
        st.selected_pos = 0;
        st.scroll_offset = 0;
        recompute_filtered(st);
    });
    apply_window_layout(false);
}

unsafe fn handle_nav_key(vk: u32) -> bool {
    let ctrl = GetKeyState(VK_CONTROL as i32) < 0;
    let alt = GetKeyState(VK_MENU as i32) < 0;

    if vk == VK_ESCAPE as u32 {
        if get_recording_target() != 0 {
            clear_recording_target();
            InvalidateRect(get_popup_hwnd(), null(), 0);
            return true;
        }
        let in_settings = with_ui(|s| s.mode == ConsoleMode::Settings).unwrap_or(false);
        if in_settings {
            // Do NOT invoke AEGP_ExecuteScript when returning from Settings on Esc!
            switch_mode_inline(ConsoleMode::ApplyFx, false);
        } else {
            hide_popup_and_focus_ae();
        }
        return true;
    }

    if vk == VK_TAB as u32 {
        let cur_mode = with_ui(|s| s.mode).unwrap_or(ConsoleMode::ApplyFx);
        if ctrl && cur_mode == ConsoleMode::UsedEffects {
            toggle_filter_or_scope_inline();
        } else {
            let next = if cur_mode == ConsoleMode::ApplyFx {
                ConsoleMode::UsedEffects
            } else {
                ConsoleMode::ApplyFx
            };
            let need_ae_scan = next == ConsoleMode::UsedEffects;
            switch_mode_inline(next, need_ae_scan);
        }
        return true;
    }

    if (alt && vk == b'C' as u32) || (ctrl && vk == b'D' as u32) {
        if with_ui(|s| s.mode == ConsoleMode::UsedEffects).unwrap_or(false) {
            toggle_filter_or_scope_inline();
            return true;
        }
    }

    if alt && vk == b'F' as u32 {
        if with_ui(|s| s.mode == ConsoleMode::ApplyFx).unwrap_or(false) {
            toggle_filter_or_scope_inline();
            return true;
        }
    }

    if ctrl && vk == b'S' as u32 {
        let pos = with_ui(|s| s.selected_pos).unwrap_or(0);
        toggle_star_at_filtered_pos(pos);
        return true;
    }

    if alt && (b'1' as u32..=b'9' as u32).contains(&vk) {
        let target_pos = (vk - b'1' as u32) as usize;
        let valid = with_ui(|st| {
            if target_pos < st.filtered_indices.len() {
                st.selected_pos = target_pos;
                true
            } else {
                false
            }
        })
        .unwrap_or(false);
        if valid {
            execute_selected(false);
            return true;
        }
    }

    if vk == VK_DOWN as u32 || vk == VK_UP as u32 || vk == VK_NEXT as u32 || vk == VK_PRIOR as u32 {
        let changed = with_ui(|st| {
            let len = st.filtered_indices.len();
            if len > 0 {
                if vk == VK_DOWN as u32 {
                    st.selected_pos = (st.selected_pos + 1) % len;
                } else if vk == VK_UP as u32 {
                    st.selected_pos = (st.selected_pos + len - 1) % len;
                } else if vk == VK_NEXT as u32 {
                    st.selected_pos = (st.selected_pos + MAX_VISIBLE_ROWS).min(len - 1);
                } else if vk == VK_PRIOR as u32 {
                    st.selected_pos = st.selected_pos.saturating_sub(MAX_VISIBLE_ROWS);
                }
                ensure_selection_in_scroll(st);
                true
            } else {
                false
            }
        })
        .unwrap_or(false);
        if changed {
            InvalidateRect(get_popup_hwnd(), null(), 0);
        }
        return true;
    }

    if vk == VK_RETURN as u32 {
        execute_selected(alt);
        return true;
    }

    false
}

fn map_hit_to_hover(act: HitAction) -> HoverTarget {
    match act {
        HitAction::TabFx => HoverTarget::TabFx,
        HitAction::TabUsed => HoverTarget::TabUsed,
        HitAction::ToggleFilterOrScope => HoverTarget::FilterOrScope,
        HitAction::ToggleSettings => HoverTarget::SettingsBtn,
        HitAction::ScrollbarThumb | HitAction::ScrollbarTrack => HoverTarget::ScrollbarThumb,
        HitAction::RowClick(pos) | HitAction::StarClick(pos) => HoverTarget::Row(pos),
        HitAction::ToggleLanguage => HoverTarget::SettingsItem(0),
        HitAction::StartRecordFxHotkey => HoverTarget::SettingsItem(1),
        HitAction::StartRecordUsedHotkey => HoverTarget::SettingsItem(2),
        HitAction::ToggleDefaultScope => HoverTarget::SettingsItem(3),
        HitAction::RescanCatalog => HoverTarget::SettingsItem(4),
        HitAction::OpenConfigFolder => HoverTarget::SettingsItem(5),
        HitAction::ChangeConfigFolder => HoverTarget::SettingsItem(6),
        HitAction::DragHeader => HoverTarget::None,
    }
}

unsafe fn pick_custom_config_folder(owner_hwnd: HWND, is_en: bool) -> Option<std::path::PathBuf> {
    MODAL_DIALOG_OPEN.store(true, Ordering::SeqCst);
    let title = if is_en {
        to_wide("Select folder to store FXSearch settings and cache:")
    } else {
        to_wide("请选择用于保存 FXSearch 配置文件与缓存的文件夹：")
    };
    let mut display_buf = [0u16; 260];
    let bi = BROWSEINFOW {
        hwndOwner: owner_hwnd,
        pidlRoot: null_mut(),
        pszDisplayName: display_buf.as_mut_ptr(),
        lpszTitle: title.as_ptr(),
        ulFlags: BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE,
        lpfn: None,
        lParam: 0,
        iImage: 0,
    };

    let pidl = SHBrowseForFolderW(&bi);
    let mut out_path = None;
    if !pidl.is_null() {
        let mut path_buf = [0u16; 520];
        if SHGetPathFromIDListW(pidl, path_buf.as_mut_ptr()) != 0 {
            let len = path_buf.iter().position(|&c| c == 0).unwrap_or(0);
            if len > 0 {
                let path_str = String::from_utf16_lossy(&path_buf[..len]);
                out_path = Some(std::path::PathBuf::from(path_str));
            }
        }
        CoTaskMemFree(pidl as *const c_void);
    }
    MODAL_DIALOG_OPEN.store(false, Ordering::SeqCst);
    let _ = SetForegroundWindow(owner_hwnd);
    let _ = SetFocus(owner_hwnd);
    out_path
}

unsafe extern "system" fn edit_subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let res = catch_unwind(AssertUnwindSafe(|| -> Option<LRESULT> {
        if msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN {
            if handle_nav_key(wparam as u32) {
                return Some(0);
            }
        }
        if msg == WM_CHAR || msg == WM_SYSCHAR {
            let ch = wparam as u32;
            if ch == 0x0D || ch == 0x09 || ch == 0x1B || msg == WM_SYSCHAR {
                return Some(0);
            }
        }
        None
    }));

    if let Ok(Some(ret)) = res {
        return ret;
    }

    let orig = EDIT_ORIG_WNDPROC.load(Ordering::SeqCst);
    let orig_fn: Option<unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT> =
        std::mem::transmute(orig);
    CallWindowProcW(orig_fn, hwnd, msg, wparam, lparam)
}

unsafe extern "system" fn popup_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let result = catch_unwind(AssertUnwindSafe(|| -> LRESULT {
        match msg {
            WM_ERASEBKGND => 1,

            WM_PAINT => {
                let mut ps: PAINTSTRUCT = zeroed();
                let hdc = BeginPaint(hwnd, &mut ps);
                paint_popup(hwnd, hdc);
                EndPaint(hwnd, &ps);
                0
            }

            WM_CTLCOLOREDIT => {
                let hdc = wparam as HDC;
                SetBkColor(hdc, COL_BG_SEARCH);
                SetTextColor(hdc, COL_TEXT_MAIN);
                SEARCH_BG_BRUSH_ATOMIC.load(Ordering::SeqCst)
            }

            WM_COMMAND => {
                let notify_code = ((wparam >> 16) & 0xFFFF) as u32;
                if notify_code == EN_CHANGE && !SUPPRESS_EDIT_CHANGE.load(Ordering::SeqCst) {
                    let edit_hwnd = EDIT_HWND_ATOMIC.load(Ordering::SeqCst) as HWND;
                    if !edit_hwnd.is_null() {
                        let len = GetWindowTextLengthW(edit_hwnd);
                        let mut buf = vec![0u16; (len as usize) + 2];
                        if len > 0 {
                            GetWindowTextW(edit_hwnd, buf.as_mut_ptr(), len + 1);
                        }
                        let text = String::from_utf16_lossy(&buf[..len as usize]);

                        let is_fx_mode =
                            with_ui(|s| s.mode == ConsoleMode::ApplyFx).unwrap_or(false);
                        if is_fx_mode && text.starts_with('@') {
                            let rest = text[1..].to_string();
                            set_edit_text_silently(&rest);
                            with_ui(|st| st.query = rest);
                            switch_mode_inline(ConsoleMode::UsedEffects, true);
                        } else {
                            with_ui(|st| {
                                st.query = text;
                                st.toast_error.clear();
                                st.selected_pos = 0;
                                st.scroll_offset = 0;
                                recompute_filtered(st);
                            });
                            apply_window_layout(false);
                        }
                    }
                }
                0
            }

            WM_KEYDOWN | WM_SYSKEYDOWN => {
                if handle_nav_key(wparam as u32) {
                    0
                } else {
                    DefWindowProcW(hwnd, msg, wparam, lparam)
                }
            }

            WM_MOUSEWHEEL => {
                let delta = ((wparam >> 16) as i16) as i32;
                with_ui(|st| {
                    let max_off = st.filtered_indices.len().saturating_sub(MAX_VISIBLE_ROWS);
                    if delta < 0 {
                        st.scroll_offset = (st.scroll_offset + 2).min(max_off);
                    } else if delta > 0 {
                        st.scroll_offset = st.scroll_offset.saturating_sub(2);
                    }
                });
                InvalidateRect(hwnd, null(), 0);
                0
            }

            WM_MOUSEMOVE => {
                let x = (lparam & 0xFFFF) as i16 as i32;
                let y = ((lparam >> 16) & 0xFFFF) as i16 as i32;

                let need_track = with_ui(|st| {
                    if !st.tracking_mouse_leave {
                        st.tracking_mouse_leave = true;
                        true
                    } else {
                        false
                    }
                })
                .unwrap_or(false);

                if need_track {
                    let mut tme = TRACKMOUSEEVENT {
                        cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                        dwFlags: TME_LEAVE,
                        hwndTrack: hwnd,
                        dwHoverTime: 0,
                    };
                    let _ = TrackMouseEvent(&mut tme);
                }

                // 1. Handle active scrollbar thumb dragging
                let is_dragging_sb = with_ui(|st| {
                    if !st.dragging_scrollbar {
                        return false;
                    }
                    let max_scroll = st.filtered_indices.len().saturating_sub(MAX_VISIBLE_ROWS);
                    let avail_travel = (st.sb_track_h - st.sb_thumb_h).max(1);
                    let rel_y = (y - st.sb_track_top - st.drag_grab_offset_y)
                        .clamp(0, avail_travel);
                    let new_off =
                        ((rel_y as usize * max_scroll + (avail_travel as usize / 2)) / avail_travel as usize)
                            .min(max_scroll);
                    st.scroll_offset = new_off;
                    true
                })
                .unwrap_or(false);

                if is_dragging_sb {
                    SetCursor(LoadCursorW(null_mut(), IDC_HAND));
                    InvalidateRect(hwnd, null(), 0);
                    return 0;
                }

                // 2. Hit-test hover state across top buttons, scrollbar, settings, and rows
                let (changed, is_interactive) = with_ui(|st| {
                    let mut hit_act: Option<HitAction> = None;
                    for hb in st.hit_boxes.iter().rev() {
                        if pt_in_rect(&hb.rect, x, y) {
                            hit_act = Some(hb.action);
                            break;
                        }
                    }
                    let new_hov = hit_act.map(map_hit_to_hover).unwrap_or(HoverTarget::None);
                    let clickable = matches!(
                        hit_act,
                        Some(
                            HitAction::TabFx
                                | HitAction::TabUsed
                                | HitAction::ToggleFilterOrScope
                                | HitAction::ToggleSettings
                                | HitAction::ScrollbarThumb
                                | HitAction::ScrollbarTrack
                                | HitAction::StarClick(_)
                                | HitAction::RowClick(_)
                                | HitAction::ToggleLanguage
                                | HitAction::StartRecordFxHotkey
                                | HitAction::StartRecordUsedHotkey
                                | HitAction::ToggleDefaultScope
                                | HitAction::RescanCatalog
                                | HitAction::OpenConfigFolder
                                | HitAction::ChangeConfigFolder
                        )
                    );

                    if new_hov != st.hover {
                        st.hover = new_hov;
                        if let HoverTarget::Row(pos) = new_hov {
                            st.selected_pos = pos;
                        }
                        (true, clickable)
                    } else {
                        (false, clickable)
                    }
                })
                .unwrap_or((false, false));

                SetCursor(LoadCursorW(
                    null_mut(),
                    if is_interactive { IDC_HAND } else { IDC_ARROW },
                ));

                if changed {
                    InvalidateRect(hwnd, null(), 0);
                }
                0
            }

            WM_MOUSELEAVE => {
                let changed = with_ui(|st| {
                    st.tracking_mouse_leave = false;
                    if st.hover != HoverTarget::None {
                        st.hover = HoverTarget::None;
                        true
                    } else {
                        false
                    }
                })
                .unwrap_or(false);
                if changed {
                    InvalidateRect(hwnd, null(), 0);
                }
                0
            }

            WM_LBUTTONUP => {
                let was_dragging = with_ui(|st| {
                    if st.dragging_scrollbar {
                        st.dragging_scrollbar = false;
                        true
                    } else {
                        false
                    }
                })
                .unwrap_or(false);
                if was_dragging {
                    ReleaseCapture();
                    InvalidateRect(hwnd, null(), 0);
                }
                0
            }

            WM_LBUTTONDOWN => {
                let x = (lparam & 0xFFFF) as i16 as i32;
                let y = ((lparam >> 16) & 0xFFFF) as i16 as i32;
                let alt = GetKeyState(VK_MENU as i32) < 0;

                let hit = with_ui(|st| -> Option<(HitAction, RECT)> {
                    for hb in st.hit_boxes.iter().rev() {
                        if pt_in_rect(&hb.rect, x, y) {
                            return Some((hb.action, hb.rect));
                        }
                    }
                    None
                })
                .flatten();

                let edit_hwnd = EDIT_HWND_ATOMIC.load(Ordering::SeqCst) as HWND;

                if let Some((act, hit_rect)) = hit {
                    match act {
                        HitAction::ScrollbarThumb => {
                            with_ui(|st| {
                                st.dragging_scrollbar = true;
                                st.drag_grab_offset_y = (y - hit_rect.top).max(0);
                            });
                            SetCapture(hwnd);
                            InvalidateRect(hwnd, null(), 0);
                        }
                        HitAction::ScrollbarTrack => {
                            with_ui(|st| {
                                let max_scroll =
                                    st.filtered_indices.len().saturating_sub(MAX_VISIBLE_ROWS);
                                let avail_travel = (st.sb_track_h - st.sb_thumb_h).max(1);
                                let grab = st.sb_thumb_h / 2;
                                let rel_y = (y - st.sb_track_top - grab).clamp(0, avail_travel);
                                st.scroll_offset = ((rel_y as usize * max_scroll
                                    + (avail_travel as usize / 2))
                                    / avail_travel as usize)
                                    .min(max_scroll);
                                st.dragging_scrollbar = true;
                                st.drag_grab_offset_y = grab;
                            });
                            SetCapture(hwnd);
                            InvalidateRect(hwnd, null(), 0);
                        }
                        HitAction::TabFx => switch_mode_inline(ConsoleMode::ApplyFx, false),
                        HitAction::TabUsed => switch_mode_inline(ConsoleMode::UsedEffects, true),
                        HitAction::ToggleFilterOrScope => {
                            toggle_filter_or_scope_inline();
                            if !edit_hwnd.is_null() {
                                let _ = SetFocus(edit_hwnd);
                            }
                        }
                        HitAction::ToggleSettings => {
                            let cur = with_ui(|s| s.mode).unwrap_or(ConsoleMode::ApplyFx);
                            if cur == ConsoleMode::Settings {
                                switch_mode_inline(ConsoleMode::ApplyFx, false);
                            } else {
                                switch_mode_inline(ConsoleMode::Settings, false);
                            }
                        }
                        HitAction::StarClick(pos) => {
                            toggle_star_at_filtered_pos(pos);
                            if !edit_hwnd.is_null() {
                                let _ = SetFocus(edit_hwnd);
                            }
                        }
                        HitAction::RowClick(pos) => {
                            with_ui(|st| st.selected_pos = pos);
                            execute_selected(alt);
                        }
                        HitAction::ToggleLanguage => {
                            clear_recording_target();
                            with_ui(|st| {
                                st.settings.language = if st.settings.language == "en" {
                                    "zh".to_string()
                                } else {
                                    "en".to_string()
                                };
                                save_settings(&st.settings);
                            });
                            InvalidateRect(hwnd, null(), 0);
                        }
                        HitAction::StartRecordFxHotkey => {
                            let cur = get_recording_target();
                            RECORDING_TARGET.store(if cur == 1 { 0 } else { 1 }, Ordering::SeqCst);
                            InvalidateRect(hwnd, null(), 0);
                        }
                        HitAction::StartRecordUsedHotkey => {
                            let cur = get_recording_target();
                            RECORDING_TARGET.store(if cur == 2 { 0 } else { 2 }, Ordering::SeqCst);
                            InvalidateRect(hwnd, null(), 0);
                        }
                        HitAction::ToggleDefaultScope => {
                            clear_recording_target();
                            with_ui(|st| {
                                st.settings.default_used_scope =
                                    if st.settings.default_used_scope == "active" {
                                        "all".to_string()
                                    } else {
                                        "active".to_string()
                                    };
                                st.used_scope = if st.settings.default_used_scope == "active" {
                                    UsedScope::ActiveComp
                                } else {
                                    UsedScope::AllProject
                                };
                                save_settings(&st.settings);
                            });
                            InvalidateRect(hwnd, null(), 0);
                        }
                        HitAction::RescanCatalog => {
                            clear_recording_target();
                            let dirs = with_ui(|st| {
                                st.effects_synced = false;
                                st.settings.custom_preset_dirs.clone()
                            })
                            .unwrap_or_default();
                            let _ = try_sync_effects_on_idle();
                            let scanned = scan_all_presets(&dirs);
                            with_ui(|st| {
                                st.presets = scanned;
                                st.presets_scanned = true;
                                st.catalog = build_catalog(&st.effects, &st.presets, &st.settings);
                                recompute_filtered(st);
                            });
                            InvalidateRect(hwnd, null(), 0);
                        }
                        HitAction::OpenConfigFolder => {
                            clear_recording_target();
                            let _ = Command::new("explorer.exe").arg(get_app_data_dir()).spawn();
                        }
                        HitAction::ChangeConfigFolder => {
                            clear_recording_target();
                            let is_en = with_ui(|s| s.settings.language == "en").unwrap_or(false);
                            if let Some(chosen_dir) = pick_custom_config_folder(hwnd, is_en) {
                                if set_custom_app_data_dir(&chosen_dir) {
                                    let loaded_settings = load_settings();
                                    let loaded_usage = load_usage();
                                    with_ui(|st| {
                                        st.settings = loaded_settings;
                                        st.fx_shortcut = parse_shortcut(&st.settings.hotkey_fx);
                                        st.used_shortcut = parse_shortcut(&st.settings.hotkey_used);
                                        sync_shared_shortcuts(st.fx_shortcut, st.used_shortcut);
                                        st.usage = loaded_usage;
                                        st.catalog =
                                            build_catalog(&st.effects, &st.presets, &st.settings);
                                        recompute_filtered(st);
                                    });
                                }
                            }
                            InvalidateRect(hwnd, null(), 0);
                        }
                        HitAction::DragHeader => {
                            clear_recording_target();
                            ReleaseCapture();
                            SendMessageW(hwnd, WM_NCLBUTTONDOWN, HTCAPTION as usize, 0);
                        }
                    }
                } else {
                    clear_recording_target();
                    InvalidateRect(hwnd, null(), 0);
                }
                0
            }

            WM_ACTIVATE => {
                let state_code = (wparam & 0xFFFF) as u32;
                let activated_hwnd = lparam as HWND;
                if state_code == WA_INACTIVE
                    && is_popup_visible()
                    && !is_modal_dialog_open()
                    && activated_hwnd != hwnd
                {
                    let elapsed_ms =
                        with_ui(|s| s.opened_at.elapsed().as_millis()).unwrap_or(999);
                    if elapsed_ms > 80 {
                        hide_popup_without_stealing_click();
                    }
                }
                0
            }

            WM_APP_HIDE_POPUP => {
                let dragging = with_ui(|s| s.dragging_scrollbar).unwrap_or(false);
                if is_popup_visible() && !dragging && !is_modal_dialog_open() {
                    hide_popup_without_stealing_click();
                }
                0
            }

            WM_APP_RECORD_KEY => {
                let vk = wparam as u32;
                let mods = lparam as usize;
                let target = get_recording_target();
                if vk == VK_ESCAPE as u32 {
                    clear_recording_target();
                    InvalidateRect(hwnd, null(), 0);
                } else if target == 1 || target == 2 {
                    let ctrl = (mods & 1) != 0;
                    let shift = (mods & 2) != 0;
                    let alt = (mods & 4) != 0;
                    if let Some(spec) = format_shortcut_spec(ctrl, shift, alt, vk) {
                        clear_recording_target();
                        with_ui(|st| {
                            if target == 1 {
                                st.settings.hotkey_fx = spec.clone();
                                st.fx_shortcut = parse_shortcut(&spec);
                            } else {
                                st.settings.hotkey_used = spec.clone();
                                st.used_shortcut = parse_shortcut(&spec);
                            }
                            sync_shared_shortcuts(st.fx_shortcut, st.used_shortcut);
                            save_settings(&st.settings);
                        });
                        InvalidateRect(hwnd, null(), 0);
                    }
                }
                0
            }

            WM_APP_TOGGLE_FX => {
                open_popup_in_mode(ConsoleMode::ApplyFx, true);
                0
            }
            WM_APP_TOGGLE_USED => {
                open_popup_in_mode(ConsoleMode::UsedEffects, true);
                0
            }
            WM_APP_OPEN_FX => {
                open_popup_in_mode(ConsoleMode::ApplyFx, false);
                0
            }
            WM_APP_OPEN_USED => {
                open_popup_in_mode(ConsoleMode::UsedEffects, false);
                0
            }

            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }));

    result.unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_sys::Win32::Graphics::Gdi::{GetDC, ReleaseDC};

    #[test]
    fn test_custom_shortcut_formatter_and_parser() {
        let spec = format_shortcut_spec(true, false, true, b'Q' as u32).unwrap();
        assert_eq!(spec, "Ctrl+Alt+Q");
        let parsed = parse_shortcut(&spec);
        assert!(parsed.ctrl && !parsed.shift && parsed.alt && parsed.vk == b'Q' as u32);

        let fn_spec = format_shortcut_spec(false, false, false, 0x73).unwrap();
        assert_eq!(fn_spec, "F4");
        let fn_parsed = parse_shortcut(&fn_spec);
        assert!(!fn_parsed.ctrl && fn_parsed.vk == 0x73);

        assert!(format_shortcut_spec(false, false, false, b'A' as u32).is_none());
    }

    #[test]
    fn test_vector_icons_scrollbar_hover_and_esc_from_settings() {
        unsafe {
            ensure_ui_initialized();
            let popup = get_popup_hwnd();
            let edit = EDIT_HWND_ATOMIC.load(Ordering::SeqCst) as HWND;
            assert!(!popup.is_null());
            assert!(!edit.is_null());

            SendMessageW(popup, WM_APP_TOGGLE_FX, 0, 0);
            assert!(is_popup_visible());

            // Paint FX mode with scrollbar + icons
            let hdc = GetDC(popup);
            paint_popup(popup, hdc);
            ReleaseDC(popup, hdc);

            // Switch to Used Effects mode & paint target icons + neutral scope pill
            switch_mode_inline(ConsoleMode::UsedEffects, false);
            let hdc2 = GetDC(popup);
            paint_popup(popup, hdc2);
            ReleaseDC(popup, hdc2);

            // Open Settings & press Esc -> must return to ApplyFx cleanly without script call
            switch_mode_inline(ConsoleMode::Settings, false);
            SendMessageW(popup, WM_KEYDOWN, VK_ESCAPE as usize, 0);
            let mode_after = with_ui(|s| s.mode).unwrap();
            assert_eq!(mode_after, ConsoleMode::ApplyFx);

            SendMessageW(popup, WM_APP_HIDE_POPUP, 0, 0);
            assert!(!is_popup_visible());
        }
    }
}
