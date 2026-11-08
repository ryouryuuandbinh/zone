use std::{ffi::c_void, mem::size_of, ptr, slice};

use windows_sys::Win32::{
    Foundation::RECT,
    Graphics::{
        Dwm::DwmFlush,
        Gdi::{
            BitBlt, CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GdiFlush, GetDC,
            GetMonitorInfoW, MonitorFromRect, ReleaseDC, SelectObject, BITMAPINFO,
            BITMAPINFOHEADER, BI_RGB, CAPTUREBLT, DIB_RGB_COLORS, HBITMAP, HDC, HGDIOBJ,
            MONITORINFO, MONITOR_DEFAULTTONULL, SRCCOPY,
        },
    },
};

use super::{pixel_count, CapturedFrame, ScreenBounds};

struct ScreenDc(HDC);

impl Drop for ScreenDc {
    fn drop(&mut self) {
        // SAFETY: This owned desktop DC is released on the same worker that acquired it.
        unsafe { ReleaseDC(ptr::null_mut(), self.0) };
    }
}

struct MemoryDc(HDC);

impl Drop for MemoryDc {
    fn drop(&mut self) {
        // SAFETY: This DC was created with CreateCompatibleDC and is owned here.
        unsafe { DeleteDC(self.0) };
    }
}

struct Bitmap(HBITMAP);

impl Drop for Bitmap {
    fn drop(&mut self) {
        // SAFETY: The selection guard restores the previous object before this bitmap drops.
        unsafe { DeleteObject(self.0) };
    }
}

struct SelectedBitmap<'a> {
    dc: &'a MemoryDc,
    previous: HGDIOBJ,
    _bitmap: &'a Bitmap,
}

impl Drop for SelectedBitmap<'_> {
    fn drop(&mut self) {
        // SAFETY: Both objects remain alive and the previous object belongs to this DC.
        unsafe { SelectObject(self.dc.0, self.previous) };
    }
}

fn validate_monitor(bounds: ScreenBounds) -> Result<(), String> {
    let rect = RECT {
        left: bounds.x,
        top: bounds.y,
        right: bounds
            .x
            .checked_add_unsigned(bounds.width)
            .ok_or("Vị trí màn hình không hợp lệ.")?,
        bottom: bounds
            .y
            .checked_add_unsigned(bounds.height)
            .ok_or("Vị trí màn hình không hợp lệ.")?,
    };
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: The rectangle and correctly sized output structure remain live for both calls.
    let unchanged = unsafe {
        let monitor = MonitorFromRect(&rect, MONITOR_DEFAULTTONULL);
        !monitor.is_null() && GetMonitorInfoW(monitor, &mut info) != 0
    };
    let monitor = info.rcMonitor;
    if !unchanged
        || (monitor.left, monitor.top, monitor.right, monitor.bottom)
            != (rect.left, rect.top, rect.right, rect.bottom)
    {
        return Err("Màn hình đã thay đổi. Hãy thử chụp lại.".into());
    }
    Ok(())
}

pub(super) fn capture(bounds: ScreenBounds) -> Result<CapturedFrame, String> {
    let count = pixel_count(bounds.width, bounds.height)?;
    let width = bounds.width as i32;
    let height = bounds.height as i32;
    let dib_bytes = count * 4;

    // SAFETY: DwmFlush takes no pointers. This is a blocking worker, not the UI thread.
    if unsafe { DwmFlush() } < 0 {
        return Err("Windows không thể đồng bộ màn hình để chụp.".into());
    }
    validate_monitor(bounds)?;

    // SAFETY: A null HWND requests the screen DC. The matching guard releases it here.
    let screen = unsafe { GetDC(ptr::null_mut()) };
    if screen.is_null() {
        return Err("Windows không thể mở màn hình để chụp.".into());
    }
    let screen = ScreenDc(screen);

    // SAFETY: The screen DC remains alive for the compatibility query.
    let memory = unsafe { CreateCompatibleDC(screen.0) };
    if memory.is_null() {
        return Err("Windows không thể tạo vùng nhớ để chụp màn hình.".into());
    }
    let memory = MemoryDc(memory);
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            biSizeImage: dib_bytes as u32,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bits: *mut c_void = ptr::null_mut();
    // SAFETY: The 32-bit top-down DIB has no palette and GDI owns its backing allocation.
    let bitmap = unsafe {
        CreateDIBSection(
            screen.0,
            &info,
            DIB_RGB_COLORS,
            &mut bits,
            ptr::null_mut(),
            0,
        )
    };
    if bitmap.is_null() {
        return Err("Windows không thể cấp bộ nhớ cho ảnh chụp.".into());
    }
    let bitmap = Bitmap(bitmap);
    if bits.is_null() {
        return Err("Windows không trả dữ liệu ảnh chụp.".into());
    }

    // SAFETY: The selected bitmap is owned, and only selected into this private memory DC.
    let previous = unsafe { SelectObject(memory.0, bitmap.0) };
    if previous.is_null() {
        return Err("Windows không thể chuẩn bị ảnh chụp.".into());
    }
    let _selection = SelectedBitmap {
        dc: &memory,
        previous,
        _bitmap: &bitmap,
    };
    // SAFETY: Each row is width * 4 bytes in this DIB. Initialize reserved bytes as well
    // as the pixel channels, then finish GDI work before reading the buffer.
    let transferred = unsafe {
        ptr::write_bytes(bits.cast::<u8>(), 0, dib_bytes);
        BitBlt(
            memory.0,
            0,
            0,
            width,
            height,
            screen.0,
            bounds.x,
            bounds.y,
            SRCCOPY | CAPTUREBLT,
        ) != 0
            && GdiFlush() != 0
    };
    if !transferred {
        return Err("Windows không thể lấy ảnh từ màn hình này.".into());
    }

    let mut rgb = Vec::new();
    rgb.try_reserve_exact(count * 3)
        .map_err(|_| "Không đủ bộ nhớ cho ảnh chụp.")?;
    // SAFETY: BitBlt and GdiFlush completed. The initialized DIB allocation still lives,
    // and the slice ends before the bitmap guard releases it.
    let source = unsafe { slice::from_raw_parts(bits.cast::<u8>(), dib_bytes) };
    for pixel in source.as_chunks::<4>().0 {
        rgb.extend_from_slice(&[pixel[2], pixel[1], pixel[0]]);
    }
    CapturedFrame::from_rgb(bounds.width, bounds.height, rgb)
}
