//! Event-driven native hit testing without clipping WebView2's antialiased pixels.
//!
//! The low-level observer reads only cursor coordinates. It never swallows,
//! injects, stores, or logs input. Style changes run on the owning GUI thread
//! through a bounded message, so a stalled GUI cannot stall the mouse hook.

use crate::windows::geometry::Spring;
use std::{
    cell::Cell,
    ptr,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use tauri::WebviewWindow;
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
    System::{LibraryLoader::GetModuleHandleW, Threading::GetCurrentThreadId},
    UI::{HiDpi::GetDpiForWindow, WindowsAndMessaging::*},
};

pub const APPLY_HIT_STATE: u32 = WM_APP + 0x245;
pub const REQUEST_HIT_REFRESH: u32 = WM_APP + 0x247;
const REFRESH_CURSOR: u32 = WM_APP + 0x246;
const HIT_STYLES: isize = (WS_EX_TRANSPARENT | WS_EX_LAYERED) as isize;

thread_local! {
    static HOOK_CONTEXT: Cell<*const HitContext> = const { Cell::new(ptr::null()) };
}

struct HitContext {
    hwnd: HWND,
    spring: Arc<Mutex<Spring>>,
}

impl HitContext {
    fn refresh(&self, point: Option<POINT>) {
        // Never wait on application locks from the hook callback.
        let geometry = match self.spring.try_lock() {
            Ok(spring) => spring.sample(Instant::now()).0,
            Err(_) => return,
        };
        let mut cursor = point.unwrap_or(POINT { x: 0, y: 0 });
        let mut rect = RECT::default();
        // SAFETY: This observer is scoped to an application-owned HWND. The
        // coordinate buffers are live stack values, and no foreign memory is read.
        unsafe {
            if IsWindow(self.hwnd) == 0 {
                return;
            }
            if point.is_none() && GetCursorPos(&mut cursor) == 0 {
                return;
            }
            if GetWindowRect(self.hwnd, &mut rect) == 0 {
                return;
            }
            let scale = (GetDpiForWindow(self.hwnd) as f64 / 96.0).max(1.0);
            let inside = IsWindowVisible(self.hwnd) != 0
                && geometry.contains_canvas_point(
                    f64::from(cursor.x - rect.left) / scale,
                    f64::from(cursor.y - rect.top) / scale,
                );
            let style = GetWindowLongPtrW(self.hwnd, GWL_EXSTYLE);
            let desired = if inside {
                WS_EX_LAYERED as isize
            } else {
                HIT_STYLES
            };
            // Check actual styles, not a cached boolean: Tao can rewrite them.
            if style & HIT_STYLES == desired {
                return;
            }
            let mut result = 0;
            let delivered = SendMessageTimeoutW(
                self.hwnd,
                APPLY_HIT_STATE,
                usize::from(!inside),
                0,
                SMTO_ABORTIFHUNG | SMTO_BLOCK,
                2,
                &mut result,
            );
            if delivered == 0 {
                // A payload-free refresh is safe if the GUI was busy. Recompute
                // from the latest cursor on a later hook event/animation tick.
                let _ = PostMessageW(self.hwnd, REQUEST_HIT_REFRESH, 0, 0);
            }
        }
    }
}

unsafe extern "system" fn observe_mouse(code: i32, _message: WPARAM, data: LPARAM) -> LRESULT {
    if code >= 0 && data != 0 {
        HOOK_CONTEXT.with(|slot| {
            let context = slot.get();
            if !context.is_null() {
                // SAFETY: Windows supplies MSLLHOOKSTRUCT for this callback.
                // Context is owned by this same thread until after unhooking.
                unsafe {
                    (&*context).refresh(Some((*(data as *const MSLLHOOKSTRUCT)).pt));
                }
            }
        });
    }
    // SAFETY: Always forward unchanged input, including negative hook codes.
    unsafe { CallNextHookEx(ptr::null_mut(), code, _message, data) }
}

pub struct MouseHitTest {
    thread_id: u32,
    stopped: AtomicBool,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl MouseHitTest {
    pub fn new(window: &WebviewWindow, spring: Arc<Mutex<Spring>>) -> Result<Self, String> {
        let hwnd = window.hwnd().map_err(|error| error.to_string())?.0 as usize;
        let (ready, receive) = mpsc::sync_channel(1);
        let worker = thread::Builder::new()
            .name("zone-mouse-hit-test".into())
            .spawn(move || {
                let context = HitContext {
                    hwnd: hwnd as HWND,
                    spring,
                };
                let mut message = MSG::default();
                // SAFETY: This thread owns its queue and callback context. PeekMessage
                // creates the queue before publishing the thread ID to the owner.
                unsafe {
                    PeekMessageW(&mut message, ptr::null_mut(), 0, 0, PM_NOREMOVE);
                    HOOK_CONTEXT.with(|slot| slot.set(&context));
                    let hook = SetWindowsHookExW(
                        WH_MOUSE_LL,
                        Some(observe_mouse),
                        GetModuleHandleW(ptr::null()),
                        0,
                    );
                    if hook.is_null() {
                        HOOK_CONTEXT.with(|slot| slot.set(ptr::null()));
                        let _ = ready.send(Err(format!(
                            "Không thể khởi tạo vùng nhận chuột: {}",
                            std::io::Error::last_os_error()
                        )));
                        return;
                    }
                    if ready.send(Ok(GetCurrentThreadId())).is_ok() {
                        loop {
                            let status = GetMessageW(&mut message, ptr::null_mut(), 0, 0);
                            if status <= 0 {
                                break;
                            }
                            if message.message == REFRESH_CURSOR {
                                context.refresh(None);
                            } else {
                                TranslateMessage(&message);
                                DispatchMessageW(&message);
                            }
                        }
                    }
                    UnhookWindowsHookEx(hook);
                    HOOK_CONTEXT.with(|slot| slot.set(ptr::null()));
                }
            })
            .map_err(|error| format!("Không thể tạo luồng nhận chuột: {error}"))?;
        match receive.recv_timeout(Duration::from_secs(3)) {
            Ok(Ok(thread_id)) => Ok(Self {
                thread_id,
                stopped: AtomicBool::new(false),
                worker: Mutex::new(Some(worker)),
            }),
            Ok(Err(error)) => {
                let _ = worker.join();
                Err(error)
            }
            Err(error) => Err(format!("Vùng nhận chuột chưa sẵn sàng: {error}")),
        }
    }

    pub fn refresh(&self) -> Result<(), String> {
        if self.stopped.load(Ordering::Acquire) {
            return Ok(());
        }
        // SAFETY: ID was received only after the observer created its message queue.
        if unsafe { PostThreadMessageW(self.thread_id, REFRESH_CURSOR, 0, 0) } == 0 {
            return Err("Luồng nhận chuột đã dừng".into());
        }
        Ok(())
    }

    pub fn stop(&self) {
        if !self.stopped.swap(true, Ordering::AcqRel) {
            // SAFETY: WM_QUIT exits this owned thread's loop. Any synchronous
            // callback message is bounded to 2 ms, so GUI shutdown can join it.
            unsafe {
                PostThreadMessageW(self.thread_id, WM_QUIT, 0, 0);
            }
        }
        if let Ok(mut worker) = self.worker.lock() {
            if let Some(handle) = worker.take() {
                let _ = handle.join();
            }
        }
    }
}

impl Drop for MouseHitTest {
    fn drop(&mut self) {
        self.stop();
    }
}

pub fn set_hit_state(hwnd: HWND, ignore: bool) {
    // SAFETY: Called only from the owning GUI subclass (or initialization), never
    // directly from the low-level hook. Preserve every unrelated window style.
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        // Keep the compositing mode stable as the pointer crosses the edge.
        // Only input transparency changes; layered rendering stays enabled.
        let next = style | WS_EX_LAYERED as isize;
        let next = if ignore {
            next | WS_EX_TRANSPARENT as isize
        } else {
            next & !(WS_EX_TRANSPARENT as isize)
        };
        let next = super::windows::activation_style(next);
        if next != style {
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, next);
            SetWindowPos(
                hwnd,
                ptr::null_mut(),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
            );
        }
    }
}
