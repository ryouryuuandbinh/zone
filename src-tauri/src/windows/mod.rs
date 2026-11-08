pub mod geometry;

#[cfg(windows)]
use crate::platform::mouse::MouseHitTest;
use crate::platform::timing::AnimationTimerResolution;
use geometry::{Geometry, Spring};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

pub const CANVAS_WIDTH: f64 = 720.0;
pub const CANVAS_HEIGHT: f64 = 560.0;
pub const TOP_INSET: f64 = 12.0;

enum WorkerMessage {
    Animate,
    Stop,
}

pub struct OverlayController {
    spring: Arc<Mutex<Spring>>,
    sender: mpsc::Sender<WorkerMessage>,
    worker: Mutex<Option<JoinHandle<()>>>,
    #[cfg(windows)]
    observer: Arc<MouseHitTest>,
}

impl OverlayController {
    pub fn new(app: AppHandle) -> Result<Self, String> {
        let spring = Arc::new(Mutex::new(Spring::default()));
        let worker_spring = Arc::clone(&spring);
        #[cfg(windows)]
        let observer = Arc::new(MouseHitTest::new(
            &app.get_webview_window("island")
                .ok_or("Không tìm thấy cửa sổ Zone")?,
            Arc::clone(&spring),
        )?);
        #[cfg(windows)]
        let worker_observer = Arc::clone(&observer);
        let (sender, receiver) = mpsc::channel();
        let worker = thread::Builder::new()
            .name("zone-hit-geometry".into())
            .spawn(move || {
                let mut active = false;
                let mut timer_precision: Option<AnimationTimerResolution> = None;
                loop {
                    let message = if active {
                        receiver.recv_timeout(Duration::from_millis(8))
                    } else {
                        receiver
                            .recv()
                            .map_err(|_| mpsc::RecvTimeoutError::Disconnected)
                    };
                    match message {
                        Ok(WorkerMessage::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                            break
                        }
                        Ok(WorkerMessage::Animate) | Err(mpsc::RecvTimeoutError::Timeout) => {}
                    }
                    {
                        let Ok(mut state) = worker_spring.lock() else {
                            break;
                        };
                        active = !state.is_settled(Instant::now());
                        if !active {
                            state.settle();
                        }
                    }
                    if active {
                        timer_precision.get_or_insert_with(AnimationTimerResolution::acquire);
                    } else {
                        timer_precision.take();
                    }
                    // Native timing now updates input eligibility only. It never
                    // clips pixels, resizes HWNDs, or invalidates WebView2 rendering.
                    #[cfg(windows)]
                    if worker_observer.refresh().is_err() {
                        break;
                    }
                }
            })
            .map_err(|error| format!("Không thể tạo luồng hình học: {error}"))?;
        Ok(Self {
            spring,
            sender,
            worker: Mutex::new(Some(worker)),
            #[cfg(windows)]
            observer,
        })
    }

    pub fn set_layout(
        &self,
        geometry: Geometry,
        animate: bool,
        intensity: &str,
    ) -> Result<(), String> {
        let geometry = geometry.validate()?;
        self.spring
            .lock()
            .map_err(|_| "Không thể cập nhật hình học")?
            .retarget(geometry, animate, intensity);
        self.reapply()
    }

    pub fn reapply(&self) -> Result<(), String> {
        self.sender
            .send(WorkerMessage::Animate)
            .map_err(|_| "Luồng hình học đã dừng".into())
    }

    pub fn apply_current(&self, _window: &tauri::WebviewWindow) -> Result<(), String> {
        #[cfg(windows)]
        {
            self.observer.refresh()
        }
        #[cfg(not(windows))]
        {
            Err("Zone chỉ hỗ trợ Windows".into())
        }
    }

    pub fn stop(&self) {
        let _ = self.sender.send(WorkerMessage::Stop);
        if let Ok(mut worker) = self.worker.lock() {
            if let Some(handle) = worker.take() {
                let _ = handle.join();
            }
        }
        #[cfg(windows)]
        self.observer.stop();
    }
}

impl Drop for OverlayController {
    fn drop(&mut self) {
        self.stop();
    }
}
