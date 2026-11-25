use std::ffi::{c_char, c_void, CStr, CString};
use std::ptr::null_mut;
use std::sync::atomic::{AtomicI32, AtomicPtr, Ordering};

use after_effects_sys::{
    kAEGPCommandSuite, kAEGPCommandSuiteVersion1, kAEGPMemorySuite, kAEGPMemorySuiteVersion1,
    kAEGPRegisterSuite, kAEGPRegisterSuiteVersion5, kAEGPUtilitySuite, kAEGPUtilitySuiteVersion6,
    AEGP_Command, AEGP_CommandHook, AEGP_CommandSuite1, AEGP_ErrReportState, AEGP_HP_BeforeAE,
    AEGP_IdleHook, AEGP_MemHandle, AEGP_MemorySuite1, AEGP_MenuID, AEGP_Menu_WINDOW,
    AEGP_PluginID, AEGP_RegisterSuite5, AEGP_UpdateMenuHook, AEGP_UtilitySuite6, A_Boolean,
    SPBasicSuite, AEGP_MENU_INSERT_SORTED,
};
use serde_json::{json, Value};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_ESCAPE};

static HOST_SCRIPT_JSX: &str = include_str!("host_script.jsx");

static PICA_BASIC_PTR: AtomicPtr<SPBasicSuite> = AtomicPtr::new(null_mut());
static AEGP_PLUGIN_ID: AtomicI32 = AtomicI32::new(0);
static CMD_OPEN_FX: AtomicI32 = AtomicI32::new(0);
static CMD_OPEN_USED: AtomicI32 = AtomicI32::new(0);

pub fn init_aegp_globals(pica: *mut SPBasicSuite, plugin_id: AEGP_PluginID) {
    PICA_BASIC_PTR.store(pica, Ordering::SeqCst);
    AEGP_PLUGIN_ID.store(plugin_id, Ordering::SeqCst);
}

pub fn get_cmd_open_fx() -> AEGP_Command {
    CMD_OPEN_FX.load(Ordering::SeqCst)
}

pub fn get_cmd_open_used() -> AEGP_Command {
    CMD_OPEN_USED.load(Ordering::SeqCst)
}

unsafe fn acquire_suite<T>(name: &[u8], version: u32) -> Option<*const T> {
    let pica = PICA_BASIC_PTR.load(Ordering::SeqCst);
    if pica.is_null() {
        return None;
    }
    let acquire_fn = (*pica).AcquireSuite?;
    let mut suite_ptr: *const c_void = null_mut();
    let err = acquire_fn(name.as_ptr() as *const c_char, version as i32, &mut suite_ptr);
    if err == 0 && !suite_ptr.is_null() {
        Some(suite_ptr as *const T)
    } else {
        None
    }
}

unsafe fn release_suite(name: &[u8], version: u32) {
    let pica = PICA_BASIC_PTR.load(Ordering::SeqCst);
    if !pica.is_null() {
        if let Some(release_fn) = (*pica).ReleaseSuite {
            let _ = release_fn(name.as_ptr() as *const c_char, version as i32);
        }
    }
}

pub unsafe fn register_menus_and_hooks(
    command_hook: AEGP_CommandHook,
    update_menu_hook: AEGP_UpdateMenuHook,
    idle_hook: AEGP_IdleHook,
) {
    let plugin_id = AEGP_PLUGIN_ID.load(Ordering::SeqCst);

    if let Some(cmd_suite) = acquire_suite::<AEGP_CommandSuite1>(kAEGPCommandSuite, kAEGPCommandSuiteVersion1) {
        let mut cmd_fx: AEGP_Command = 0;
        let mut cmd_used: AEGP_Command = 0;

        if let Some(get_unique) = (*cmd_suite).AEGP_GetUniqueCommand {
            let _ = get_unique(&mut cmd_fx);
            let _ = get_unique(&mut cmd_used);
        }
        if let Some(insert_menu) = (*cmd_suite).AEGP_InsertMenuCommand {
            if cmd_fx != 0 {
                if let Ok(name) = CString::new("FXSearch") {
                    let _ = insert_menu(
                        cmd_fx,
                        name.as_ptr(),
                        AEGP_Menu_WINDOW as AEGP_MenuID,
                        AEGP_MENU_INSERT_SORTED,
                    );
                }
                CMD_OPEN_FX.store(cmd_fx, Ordering::SeqCst);
            }
            if cmd_used != 0 {
                if let Ok(name) = CString::new("FXSearch - Locate Used Effects") {
                    let _ = insert_menu(
                        cmd_used,
                        name.as_ptr(),
                        AEGP_Menu_WINDOW as AEGP_MenuID,
                        AEGP_MENU_INSERT_SORTED,
                    );
                }
                CMD_OPEN_USED.store(cmd_used, Ordering::SeqCst);
            }
        }
        release_suite(kAEGPCommandSuite, kAEGPCommandSuiteVersion1);
    }

    if let Some(reg_suite) = acquire_suite::<AEGP_RegisterSuite5>(kAEGPRegisterSuite, kAEGPRegisterSuiteVersion5) {
        let cmd_fx = CMD_OPEN_FX.load(Ordering::SeqCst);
        let cmd_used = CMD_OPEN_USED.load(Ordering::SeqCst);

        if let Some(reg_cmd) = (*reg_suite).AEGP_RegisterCommandHook {
            if cmd_fx != 0 {
                let _ = reg_cmd(
                    plugin_id,
                    AEGP_HP_BeforeAE as u32,
                    cmd_fx,
                    command_hook,
                    null_mut(),
                );
            }
            if cmd_used != 0 {
                let _ = reg_cmd(
                    plugin_id,
                    AEGP_HP_BeforeAE as u32,
                    cmd_used,
                    command_hook,
                    null_mut(),
                );
            }
        }
        if let Some(reg_menu) = (*reg_suite).AEGP_RegisterUpdateMenuHook {
            let _ = reg_menu(plugin_id, update_menu_hook, null_mut());
        }
        if let Some(reg_idle) = (*reg_suite).AEGP_RegisterIdleHook {
            let _ = reg_idle(plugin_id, idle_hook, null_mut());
        }

        release_suite(kAEGPRegisterSuite, kAEGPRegisterSuiteVersion5);
    }
}

pub unsafe fn enable_our_menu_commands() {
    if let Some(cmd_suite) = acquire_suite::<AEGP_CommandSuite1>(kAEGPCommandSuite, kAEGPCommandSuiteVersion1) {
        if let Some(enable_fn) = (*cmd_suite).AEGP_EnableCommand {
            let c1 = CMD_OPEN_FX.load(Ordering::SeqCst);
            let c2 = CMD_OPEN_USED.load(Ordering::SeqCst);
            if c1 != 0 {
                let _ = enable_fn(c1);
            }
            if c2 != 0 {
                let _ = enable_fn(c2);
            }
        }
        release_suite(kAEGPCommandSuite, kAEGPCommandSuiteVersion1);
    }
}

pub fn get_ae_main_hwnd() -> isize {
    unsafe {
        let mut hwnd_ptr: *mut c_void = null_mut();
        if let Some(util_suite) = acquire_suite::<AEGP_UtilitySuite6>(kAEGPUtilitySuite, kAEGPUtilitySuiteVersion6) {
            if let Some(get_hwnd) = (*util_suite).AEGP_GetMainHWND {
                let _ = get_hwnd(&mut hwnd_ptr as *mut *mut c_void as *mut c_void);
            }
            release_suite(kAEGPUtilitySuite, kAEGPUtilitySuiteVersion6);
        }
        hwnd_ptr as isize
    }
}

unsafe fn read_and_free_mem_handle(mem_suite: *const AEGP_MemorySuite1, handle: AEGP_MemHandle) -> String {
    if handle.is_null() {
        return String::new();
    }
    let mut out_str = String::new();
    if let Some(lock_fn) = (*mem_suite).AEGP_LockMemHandle {
        let mut raw_ptr: *mut c_void = null_mut();
        if lock_fn(handle, &mut raw_ptr) == 0 && !raw_ptr.is_null() {
            let cstr = CStr::from_ptr(raw_ptr as *const c_char);
            out_str = cstr.to_string_lossy().into_owned();
            if let Some(unlock_fn) = (*mem_suite).AEGP_UnlockMemHandle {
                let _ = unlock_fn(handle);
            }
        }
    }
    if let Some(free_fn) = (*mem_suite).AEGP_FreeMemHandle {
        let _ = free_fn(handle);
    }
    out_str
}

/// Synchronously execute an action in our embedded ExtendScript engine via `AEGP_ExecuteScript`.
/// Must be called on After Effects' main thread.
pub fn dispatch_ae_script(action: &str, payload: Value) -> Result<Value, String> {
    unsafe {
        // Never invoke ExtendScript while the physical Esc key is still held down,
        // because AE's ExtendScript VM polls VK_ESCAPE and halts with "Unable to execute script at line X"!
        if (GetAsyncKeyState(VK_ESCAPE as i32) as u16 & 0x8000) != 0 {
            return Err("Esc key held - skipped script execution".to_string());
        }

        let plugin_id = AEGP_PLUGIN_ID.load(Ordering::SeqCst);
        let util_suite = acquire_suite::<AEGP_UtilitySuite6>(kAEGPUtilitySuite, kAEGPUtilitySuiteVersion6)
            .ok_or_else(|| "无法获取 AEGP Utility Suite".to_string())?;
        let mem_suite = acquire_suite::<AEGP_MemorySuite1>(kAEGPMemorySuite, kAEGPMemorySuiteVersion1)
            .ok_or_else(|| {
                release_suite(kAEGPUtilitySuite, kAEGPUtilitySuiteVersion6);
                "无法获取 AEGP Memory Suite".to_string()
            })?;

        let result = (|| {
            if let Some(is_avail) = (*util_suite).AEGP_IsScriptingAvailable {
                let mut avail: A_Boolean = 0;
                if is_avail(&mut avail) != 0 || avail == 0 {
                    return Err("当前状态暂不可执行脚本".to_string());
                }
            }

            let exec_fn = (*util_suite)
                .AEGP_ExecuteScript
                .ok_or_else(|| "AEGP_ExecuteScript 不可用".to_string())?;

            let req_json = serde_json::to_string(&json!({
                "action": action,
                "payload": payload
            }))
            .unwrap_or_else(|_| "{}".to_string());

            let full_script = format!(
                "{}\n__FXSearchEngine.dispatch({});",
                HOST_SCRIPT_JSX, req_json
            );
            let script_cstr = CString::new(full_script).map_err(|_| "脚本包含非法空字节".to_string())?;

            let mut out_res_h: AEGP_MemHandle = null_mut();
            let mut out_err_h: AEGP_MemHandle = null_mut();

            // Suppress AE modal script error alert dialogs via AEGP_StartQuietErrors / AEGP_EndQuietErrors
            let mut err_state: AEGP_ErrReportState = std::mem::zeroed();
            let quiet_started = if let Some(start_quiet) = (*util_suite).AEGP_StartQuietErrors {
                start_quiet(&mut err_state) == 0
            } else {
                false
            };

            let err_code = exec_fn(
                plugin_id,
                script_cstr.as_ptr(),
                0, // UTF-8 encoding
                &mut out_res_h,
                &mut out_err_h,
            );

            if quiet_started {
                if let Some(end_quiet) = (*util_suite).AEGP_EndQuietErrors {
                    let _ = end_quiet(0, &mut err_state);
                }
            }

            let res_str = read_and_free_mem_handle(mem_suite, out_res_h);
            let err_str = read_and_free_mem_handle(mem_suite, out_err_h);

            if err_code != 0 || res_str.is_empty() {
                let msg = if !err_str.is_empty() {
                    err_str
                } else {
                    format!("AEGP_ExecuteScript error ({})", err_code)
                };
                return Err(msg);
            }

            serde_json::from_str::<Value>(&res_str)
                .map_err(|e| format!("解析脚本返回结果失败: {}", e))
        })();

        release_suite(kAEGPMemorySuite, kAEGPMemorySuiteVersion1);
        release_suite(kAEGPUtilitySuite, kAEGPUtilitySuiteVersion6);
        result
    }
}
