#[cfg(windows)]
pub mod mouse;
pub mod timing;
#[cfg(windows)]
mod windows;

use tauri::{LogicalSize, PhysicalPosition, WebviewWindow};

use crate::windows::{CANVAS_HEIGHT, CANVAS_WIDTH};

pub fn position_overlay(window: &WebviewWindow) -> Result<(), String> {
    let monitor = window
        .primary_monitor()
        .map_err(|error| error.to_string())?
        .ok_or("No primary monitor is available")?;
    let scale = monitor.scale_factor();
    let physical_width = (CANVAS_WIDTH * scale).round() as i32;
    let work_area = monitor.work_area();
    let x = work_area.position.x + (work_area.size.width as i32 - physical_width) / 2;
    let y = work_area.position.y;
    window
        .set_size(LogicalSize::new(CANVAS_WIDTH, CANVAS_HEIGHT))
        .map_err(|error| error.to_string())?;
    window
        .set_position(PhysicalPosition::new(x, y))
        .map_err(|error| error.to_string())?;
    if let Some(controller) = window.try_state::<crate::windows::OverlayController>() {
        controller.reapply()?;
    }
    Ok(())
}

pub fn initialize_overlay(window: &WebviewWindow) -> Result<(), String> {
    position_overlay(window)?;
    #[cfg(windows)]
    windows::initialize(window)?;
    Ok(())
}

pub fn position_settings(window: &WebviewWindow) -> Result<(), String> {
    window.center().map_err(|error| error.to_string())?;
    Ok(())
}

pub fn focus_overlay(window: &WebviewWindow) -> Result<(), String> {
    #[cfg(windows)]
    windows::prepare_focus(window)?;
    if let Err(error) = window.set_focus() {
        let _ = restore_passive_activation(window);
        return Err(error.to_string());
    }
    Ok(())
}

pub fn release_overlay_focus(window: &WebviewWindow) -> Result<(), String> {
    #[cfg(windows)]
    windows::release_focus(window)?;
    Ok(())
}

pub fn restore_passive_activation(window: &WebviewWindow) -> Result<(), String> {
    #[cfg(windows)]
    windows::set_passive_activation(window, true)?;
    Ok(())
}

use tauri::Manager;
