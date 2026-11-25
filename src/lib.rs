mod aegp_host;
mod config;
mod pinyin_util;
mod presets;
mod search;
mod ui_win32;

use std::mem::zeroed;
use std::ptr::null_mut;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::thread;

use after_effects_sys::{
    AEGP_Command, AEGP_CommandRefcon, AEGP_GlobalRefcon, AEGP_HookPriority, AEGP_IdleRefcon,
    AEGP_PluginID, AEGP_UpdateMenuRefcon, AEGP_WindowType, A_Boolean, A_Err, A_long, SPBasicSuite,
};
use windows_sys::Win32::Foundation::{LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::System::Threading::GetCurrentProcessId;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_CONTROL, VK_ESCAPE, VK_MENU, VK_SHIFT,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetAncestor, GetForegroundWindow, GetMessageW, GetWindowRect,
    GetWindowThreadProcessId, PostMessageW, SetWindowsHookExW, TranslateMessage, GA_ROOT,
    KBDLLHOOKSTRUCT, MSG, MSLLHOOKSTRUCT, WH_KEYBOARD_LL, WH_MOUSE_LL, WM_KEYDOWN, WM_KEYUP,
    WM_LBUTTONDOWN, WM_MBUTTONDOWN, WM_NCLBUTTONDOWN, WM_NCRBUTTONDOWN, WM_RBUTTONDOWN,
    WM_SYSKEYDOWN, WM_SYSKEYUP,
};

use aegp_host::{
    enable_our_menu_commands, get_cmd_open_fx, get_cmd_open_used, init_aegp_globals,
    register_menus_and_hooks,
};
use search::ConsoleMode;
use ui_win32::{
    ensure_ui_initialized, format_shortcut_spec, get_or_init_shortcuts, get_popup_hwnd,
    get_recording_target, is_modal_dialog_open, is_modifier_vk, is_popup_visible,
    open_popup_in_mode, try_sync_effects_on_idle, WM_APP_HIDE_POPUP, WM_APP_OPEN_FX,
    WM_APP_OPEN_USED, WM_APP_RECORD_KEY, WM_APP_TOGGLE_FX, WM_APP_TOGGLE_USED,
};

static HOOK_STARTED: AtomicBool = AtomicBool::new(false);
static EFFECTS_BOOT_SYNCED: AtomicBool = AtomicBool::new(false);
static OUR_AE_PID: AtomicU32 = AtomicU32::new(0);
static SWALLOWED_VK_HELD: AtomicU32 = AtomicU32::new(0);

unsafe fn is_foreground_this_ae_process() -> bool {
    let target_pid = OUR_AE_PID.load(Ordering::Relaxed);
    if target_pid == 0 {
        return false;
    }
    let fg = GetForegroundWindow();
    if fg.is_null() {
        return false;
    }
    let mut fg_pid: u32 = 0;
    GetWindowThreadProcessId(fg, &mut fg_pid);
    if fg_pid == target_pid {
        return true;
    }
    let root = GetAncestor(fg, GA_ROOT);
    if !root.is_null() && root != fg {
        let mut root_pid: u32 = 0;
        GetWindowThreadProcessId(root, &mut root_pid);
        if root_pid == target_pid {
            return true;
        }
    }
    false
}

unsafe fn key_down_async(vk: i32) -> bool {
    (GetAsyncKeyState(vk) as u16 & 0x8000) != 0
}

/// Low-level mouse hook that ONLY acts when the FXSearch popup is currently visible and a mouse click
/// occurs outside the popup's rectangle — closing FXSearch immediately while passing the click straight through to AE.
unsafe extern "system" fn ae_scoped_mouse_hook(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if code >= 0 && is_popup_visible() && !is_modal_dialog_open() {
        let msg = wparam as u32;
        if msg == WM_LBUTTONDOWN
            || msg == WM_RBUTTONDOWN
            || msg == WM_MBUTTONDOWN
            || msg == WM_NCLBUTTONDOWN
            || msg == WM_NCRBUTTONDOWN
        {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let popup = get_popup_hwnd();
                if !popup.is_null() {
                    let ms = &*(lparam as *const MSLLHOOKSTRUCT);
                    let mut rect: RECT = zeroed();
                    if GetWindowRect(popup, &mut rect) != 0 {
                        let inside = ms.pt.x >= rect.left
                            && ms.pt.x < rect.right
                            && ms.pt.y >= rect.top
                            && ms.pt.y < rect.bottom;
                        if !inside {
                            let _ = PostMessageW(popup, WM_APP_HIDE_POPUP, 0, 0);
                        }
                    }
                }
            }));
        }
    }
    CallNextHookEx(null_mut(), code, wparam, lparam)
}

unsafe extern "system" fn ae_scoped_keyboard_hook(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let handled = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> bool {
        if code >= 0 {
            let kb = &*(lparam as *const KBDLLHOOKSTRUCT);
            let vk = kb.vkCode;
            let msg = wparam as u32;

            if msg == WM_KEYUP || msg == WM_SYSKEYUP {
                if vk != 0 && SWALLOWED_VK_HELD.load(Ordering::Relaxed) == vk {
                    SWALLOWED_VK_HELD.store(0, Ordering::Relaxed);
                    return true;
                }
            } else if msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN {
                // 1. If user is actively recording a custom shortcut in Settings inside our popup
                if is_popup_visible() && get_recording_target() != 0 && is_foreground_this_ae_process() {
                    if is_modifier_vk(vk) {
                        return false;
                    }
                    let popup = get_popup_hwnd();
                    if !popup.is_null() {
                        if vk == VK_ESCAPE as u32 {
                            SWALLOWED_VK_HELD.store(vk, Ordering::Relaxed);
                            let _ = PostMessageW(popup, WM_APP_RECORD_KEY, vk as usize, 0);
                            return true;
                        }
                        let ctrl = key_down_async(VK_CONTROL as i32);
                        let shift = key_down_async(VK_SHIFT as i32);
                        let alt = key_down_async(VK_MENU as i32);
                        if format_shortcut_spec(ctrl, shift, alt, vk).is_some() {
                            SWALLOWED_VK_HELD.store(vk, Ordering::Relaxed);
                            let mods =
                                (ctrl as usize) | ((shift as usize) << 1) | ((alt as usize) << 2);
                            let _ = PostMessageW(
                                popup,
                                WM_APP_RECORD_KEY,
                                vk as usize,
                                mods as isize,
                            );
                            return true;
                        }
                    }
                }

                // 2. Normal shortcut trigger matching
                let shortcuts = get_or_init_shortcuts();
                let pair_opt = shortcuts.read().ok().map(|g| *g);
                if let Some(pair) = pair_opt {
                    if vk == pair.fx.vk || vk == pair.used.vk {
                        let ctrl = key_down_async(VK_CONTROL as i32);
                        let shift = key_down_async(VK_SHIFT as i32);
                        let alt = key_down_async(VK_MENU as i32);

                        let matched_msg = if vk == pair.used.vk
                            && ctrl == pair.used.ctrl
                            && shift == pair.used.shift
                            && alt == pair.used.alt
                        {
                            Some(WM_APP_TOGGLE_USED)
                        } else if vk == pair.fx.vk
                            && ctrl == pair.fx.ctrl
                            && shift == pair.fx.shift
                            && alt == pair.fx.alt
                        {
                            Some(WM_APP_TOGGLE_FX)
                        } else {
                            None
                        };

                        if let Some(app_msg) = matched_msg {
                            if is_foreground_this_ae_process() {
                                if SWALLOWED_VK_HELD.load(Ordering::Relaxed) != vk {
                                    SWALLOWED_VK_HELD.store(vk, Ordering::Relaxed);
                                    let popup = get_popup_hwnd();
                                    if !popup.is_null() {
                                        let _ = PostMessageW(popup, app_msg, 0, 0);
                                    }
                                }
                                return true;
                            }
                        }
                    }
                }
            }
        }
        false
    }));

    if matches!(handled, Ok(true)) {
        return 1;
    }

    CallNextHookEx(null_mut(), code, wparam, lparam)
}

fn spawn_ae_scoped_hook_thread() {
    if HOOK_STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    let _ = get_or_init_shortcuts();
    thread::spawn(|| unsafe {
        let kb_hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(ae_scoped_keyboard_hook), null_mut(), 0);
        let ms_hook = SetWindowsHookExW(WH_MOUSE_LL, Some(ae_scoped_mouse_hook), null_mut(), 0);
        if kb_hook.is_null() && ms_hook.is_null() {
            return;
        }
        let mut msg: MSG = zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    });
}

unsafe extern "C" fn command_hook(
    _plugin_refcon: AEGP_GlobalRefcon,
    _refcon: AEGP_CommandRefcon,
    command: AEGP_Command,
    _hook_priority: AEGP_HookPriority,
    _already_handled: A_Boolean,
    handled_pb: *mut A_Boolean,
) -> A_Err {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if command != 0 && command == get_cmd_open_fx() {
            let popup = get_popup_hwnd();
            if !popup.is_null() {
                let _ = PostMessageW(popup, WM_APP_OPEN_FX, 0, 0);
            } else {
                open_popup_in_mode(ConsoleMode::ApplyFx, false);
            }
            if !handled_pb.is_null() {
                *handled_pb = 1;
            }
        } else if command != 0 && command == get_cmd_open_used() {
            let popup = get_popup_hwnd();
            if !popup.is_null() {
                let _ = PostMessageW(popup, WM_APP_OPEN_USED, 0, 0);
            } else {
                open_popup_in_mode(ConsoleMode::UsedEffects, false);
            }
            if !handled_pb.is_null() {
                *handled_pb = 1;
            }
        }
    }));
    0
}

unsafe extern "C" fn update_menu_hook(
    _plugin_refcon: AEGP_GlobalRefcon,
    _refcon: AEGP_UpdateMenuRefcon,
    _active_window: AEGP_WindowType,
) -> A_Err {
    let _ = std::panic::catch_unwind(|| {
        enable_our_menu_commands();
    });
    0
}

unsafe extern "C" fn idle_hook(
    _plugin_refcon: AEGP_GlobalRefcon,
    _refcon: AEGP_IdleRefcon,
    max_sleep_pl: *mut A_long,
) -> A_Err {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if !EFFECTS_BOOT_SYNCED.load(Ordering::Relaxed) {
            if try_sync_effects_on_idle() {
                EFFECTS_BOOT_SYNCED.store(true, Ordering::Relaxed);
            }
        }
        if !max_sleep_pl.is_null() {
            *max_sleep_pl = 500;
        }
    }));
    0
}

/// Official Adobe After Effects AEGP Startup Entry Point (`Kind = 'AEgx'`, `CodeWin64X86 = "EntryPointFunc"`).
#[no_mangle]
pub unsafe extern "C" fn EntryPointFunc(
    pica_basic_p: *mut SPBasicSuite,
    _driver_major_version_l: A_long,
    _driver_minor_version_l: A_long,
    aegp_plugin_id: AEGP_PluginID,
    _global_refcon_p: *mut AEGP_GlobalRefcon,
) -> A_Err {
    let _ = std::panic::catch_unwind(|| {
        OUR_AE_PID.store(GetCurrentProcessId(), Ordering::SeqCst);
        init_aegp_globals(pica_basic_p, aegp_plugin_id);
        ensure_ui_initialized();
        register_menus_and_hooks(
            Some(command_hook),
            Some(update_menu_hook),
            Some(idle_hook),
        );
        spawn_ae_scoped_hook_thread();
    });

    0
}
