//! Windows-only overlay geometry and activation policy.
//!
//! Safety boundary: HWNDs come from a live Tauri WebviewWindow. The subclass
//! owns its reference data until WM_NCDESTROY. Rendering remains unclipped so
//! WebView2 can antialias the curved surface; native mouse handling is separate.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
use tauri::{AppHandle, Manager, WebviewWindow};
use windows_sys::Win32::Foundation::{GetLastError, SetLastError, HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowLongPtrW, IsWindow, SetForegroundWindow, SetWindowLongPtrW,
    GWL_EXSTYLE, WM_DISPLAYCHANGE, WM_NCDESTROY, WM_SETTINGCHANGE, WM_STYLECHANGED,
    WS_EX_NOACTIVATE,
};

const SUBCLASS_ID: usize = 0x5A4F4E45;
static PREVIOUS_FOREGROUND: Mutex<Option<usize>> = Mutex::new(None);
static PASSIVE_ACTIVATION: AtomicBool = AtomicBool::new(true);

pub(super) fn activation_style(style: isize) -> isize {
    if PASSIVE_ACTIVATION.load(Ordering::Relaxed) {
        style | WS_EX_NOACTIVATE as isize
    } else {
        style & !(WS_EX_NOACTIVATE as isize)
    }
}

pub fn initialize(window: &WebviewWindow) -> Result<(), String> {
    let hwnd = window.hwnd().map_err(|error| error.to_string())?.0 as HWND;
    set_passive_activation(window, true)?;
    let data = Box::into_raw(Box::new(window.app_handle().clone()));
    // SAFETY: hwnd belongs to this live Tauri window. The subclass callback owns
    // data until WM_NCDESTROY; a failed installation returns ownership here.
    unsafe {
        if SetWindowSubclass(hwnd, Some(overlay_subclass), SUBCLASS_ID, data as usize) == 0 {
            drop(Box::from_raw(data));
            return Err(format!(
                "Cannot observe Windows display changes: {}",
                std::io::Error::last_os_error()
            ));
        }
    }
    super::mouse::set_hit_state(hwnd, true);
    Ok(())
}

pub fn set_passive_activation(window: &WebviewWindow, passive: bool) -> Result<(), String> {
    PASSIVE_ACTIVATION.store(passive, Ordering::Relaxed);
    let hwnd = window.hwnd().map_err(|error| error.to_string())?.0 as HWND;
    // SAFETY: The HWND is a live application-owned window. Only NOACTIVATE is
    // changed; all other extended styles are preserved.
    unsafe {
        SetLastError(0);
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        if style == 0 && GetLastError() != 0 {
            return Err(format!(
                "Cannot read window activation style: {}",
                std::io::Error::last_os_error()
            ));
        }
        let next = if passive {
            style | WS_EX_NOACTIVATE as isize
        } else {
            style & !(WS_EX_NOACTIVATE as isize)
        };
        SetLastError(0);
        let previous = SetWindowLongPtrW(hwnd, GWL_EXSTYLE, next);
        if previous == 0 && GetLastError() != 0 {
            return Err(format!(
                "Cannot update window activation style: {}",
                std::io::Error::last_os_error()
            ));
        }
    }
    Ok(())
}

pub fn prepare_focus(window: &WebviewWindow) -> Result<(), String> {
    let hwnd = window.hwnd().map_err(|error| error.to_string())?.0 as HWND;
    // SAFETY: Querying the foreground HWND does not transfer ownership. Save only
    // a handle, never a pointer to another process's memory.
    let foreground = unsafe { GetForegroundWindow() };
    if !foreground.is_null() && foreground != hwnd {
        *PREVIOUS_FOREGROUND
            .lock()
            .map_err(|_| "Focus state is unavailable")? = Some(foreground as usize);
    }
    set_passive_activation(window, false)
}

pub fn release_focus(window: &WebviewWindow) -> Result<(), String> {
    let hwnd = window.hwnd().map_err(|error| error.to_string())?.0 as HWND;
    set_passive_activation(window, true)?;
    // SAFETY: Do not steal focus when the user has already switched apps. A
    // saved handle is revalidated with IsWindow before being passed to Windows.
    unsafe {
        if GetForegroundWindow() == hwnd {
            let previous = {
                PREVIOUS_FOREGROUND
                    .lock()
                    .map_err(|_| "Focus state is unavailable")?
                    .take()
            };
            if let Some(previous) = previous {
                let previous = previous as HWND;
                if previous != hwnd && IsWindow(previous) != 0 {
                    SetForegroundWindow(previous);
                }
            }
        }
    }
    Ok(())
}

unsafe extern "system" fn overlay_subclass(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    subclass_id: usize,
    reference_data: usize,
) -> LRESULT {
    if message == super::mouse::APPLY_HIT_STATE {
        super::mouse::set_hit_state(hwnd, wparam != 0);
        return 0;
    } else if message == super::mouse::REQUEST_HIT_REFRESH {
        // Never apply an old inside/outside decision after a timed-out send.
        // Request a fresh sample of both cursor and spring instead.
        let app = unsafe { &*(reference_data as *const AppHandle) };
        if let Some(controller) = app.try_state::<crate::windows::OverlayController>() {
            let _ = controller.reapply();
        }
        return 0;
    } else if message == WM_STYLECHANGED {
        // SAFETY: The owning GUI thread repairs only our activation bit. If a
        // nested STYLECHANGED is sent, its style already matches and stops here.
        unsafe {
            let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            let desired = activation_style(style);
            if style != desired {
                SetWindowLongPtrW(hwnd, GWL_EXSTYLE, desired);
            }
        }
        // Tao may rewrite extended styles during visibility/focus changes.
        // Recheck the current cursor asynchronously; never recurse into setters.
        let app = unsafe { &*(reference_data as *const AppHandle) };
        if let Some(controller) = app.try_state::<crate::windows::OverlayController>() {
            let _ = controller.reapply();
        }
    } else if message == WM_NCDESTROY {
        // SAFETY: This pointer was created by initialize and belongs to this
        // subclass. Removing the subclass prevents any further callback use.
        unsafe {
            RemoveWindowSubclass(hwnd, Some(overlay_subclass), subclass_id);
            drop(Box::from_raw(reference_data as *mut AppHandle));
        }
    } else if message == WM_DISPLAYCHANGE || message == WM_SETTINGCHANGE {
        // SAFETY: The reference remains live until WM_NCDESTROY. Clone it before
        // scheduling work so the closure does not retain the raw pointer.
        let app = unsafe { &*(reference_data as *const AppHandle) }.clone();
        let dispatch = app.clone();
        let _ = app.run_on_main_thread(move || {
            if let Some(window) = dispatch.get_webview_window("island") {
                if let Err(error) = super::position_overlay(&window) {
                    eprintln!("Zone display reposition failed: {error}");
                }
            }
        });
    }
    // SAFETY: Forward all messages, including destruction, to the next procedure.
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}
