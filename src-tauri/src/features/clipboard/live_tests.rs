//! Explicit opt-in integration test. Runs in a fresh noninteractive window
//! station with its own clipboard, never WinSta0 or the user's clipboard.
//! Run alone: cargo test --lib isolated_win32_clipboard -- --ignored --test-threads=1

use super::*;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use windows_sys::Win32::System::StationsAndDesktops::*;

struct PrivateStation {
    original_station: HWINSTA,
    original_desktop: HDESK,
    station: HWINSTA,
    desktop: HDESK,
}

fn native_error(action: &str) -> String {
    format!("{action}: {}", std::io::Error::last_os_error())
}

impl PrivateStation {
    fn create() -> Result<Self, String> {
        let name = format!(
            "ZoneClipboardTest-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|error| error.to_string())?
                .as_nanos()
        );
        // SAFETY: Only this dedicated test process is reassigned. No input
        // desktop is switched, no existing clipboard contents are read, and
        // CREATE_ONLY prevents accidentally opening another station's clipboard.
        unsafe {
            let original_station = GetProcessWindowStation();
            let original_desktop =
                GetThreadDesktop(windows_sys::Win32::System::Threading::GetCurrentThreadId());
            if original_station.is_null() || original_desktop.is_null() {
                return Err(native_error("Get original station/desktop"));
            }
            let access = (WINSTA_ACCESSCLIPBOARD
                | WINSTA_ACCESSGLOBALATOMS
                | WINSTA_CREATEDESKTOP
                | WINSTA_READATTRIBUTES) as u32;
            // Windows requires an administrator token for a fresh named station.
            // Never fall back to an existing station if creation is denied.
            let station =
                CreateWindowStationW(wide(&name).as_ptr(), CWF_CREATE_ONLY, access, ptr::null());
            if station.is_null() {
                return Err(native_error("Create private window station"));
            }
            let mut guard = Self {
                original_station,
                original_desktop,
                station,
                desktop: ptr::null_mut(),
            };
            if SetProcessWindowStation(station) == 0 {
                return Err(native_error("Set private window station"));
            }
            let mut flags = USEROBJECTFLAGS::default();
            let mut needed = 0;
            if GetUserObjectInformationW(
                station,
                UOI_FLAGS,
                (&mut flags as *mut USEROBJECTFLAGS).cast(),
                std::mem::size_of::<USEROBJECTFLAGS>() as u32,
                &mut needed,
            ) == 0
                || flags.dwFlags & WSF_VISIBLE as u32 != 0
                || GetProcessWindowStation() != station
            {
                return Err(
                    "Refusing clipboard access: station isolation was not confirmed".into(),
                );
            }
            guard.desktop = CreateDesktopW(
                wide("ZoneTest").as_ptr(),
                ptr::null(),
                ptr::null(),
                0,
                DESKTOP_CREATEWINDOW | DESKTOP_READOBJECTS | DESKTOP_WRITEOBJECTS,
                ptr::null(),
            );
            if guard.desktop.is_null() {
                return Err(native_error("Create private desktop"));
            }
            if SetThreadDesktop(guard.desktop) == 0 {
                return Err(native_error("Set private desktop"));
            }
            Ok(guard)
        }
    }
}

impl Drop for PrivateStation {
    fn drop(&mut self) {
        // SAFETY: All test windows have already been destroyed. Borrowed original
        // handles are restored, not closed. Only created handles are released.
        unsafe {
            if SetProcessWindowStation(self.original_station) == 0 {
                eprintln!("{}", native_error("Restore original window station"));
            }
            if SetThreadDesktop(self.original_desktop) == 0 {
                eprintln!("{}", native_error("Restore original desktop"));
            }
            if !self.desktop.is_null() {
                CloseDesktop(self.desktop);
            }
            CloseWindowStation(self.station);
        }
    }
}

struct Windows {
    class: Vec<u16>,
    owner: HWND,
    observer: HWND,
}

impl Windows {
    fn create() -> Result<Self, String> {
        let mut windows = Self {
            class: wide("ZoneIsolatedClipboardFixture"),
            owner: ptr::null_mut(),
            observer: ptr::null_mut(),
        };
        // SAFETY: This test thread owns the class and its message-only windows.
        // DefWindowProc has no custom callback pointers or foreign state.
        unsafe {
            let module = GetModuleHandleW(ptr::null());
            let class = WNDCLASSW {
                lpfnWndProc: Some(DefWindowProcW),
                hInstance: module,
                lpszClassName: windows.class.as_ptr(),
                ..Default::default()
            };
            if RegisterClassW(&class) == 0 {
                return Err(native_error("Register fixture window"));
            }
            for target in [&mut windows.owner, &mut windows.observer] {
                *target = CreateWindowExW(
                    0,
                    windows.class.as_ptr(),
                    ptr::null(),
                    0,
                    0,
                    0,
                    0,
                    0,
                    HWND_MESSAGE,
                    ptr::null_mut(),
                    module,
                    ptr::null(),
                );
                if target.is_null() {
                    return Err(native_error("Create fixture window"));
                }
            }
            if AddClipboardFormatListener(windows.observer) == 0 {
                return Err(native_error("Register clipboard update observer"));
            }
        }
        Ok(windows)
    }

    fn receives_update(&self) -> bool {
        let deadline = Instant::now() + Duration::from_secs(1);
        // A bounded test wait only, not a production polling loop.
        while Instant::now() < deadline {
            let mut message = MSG::default();
            // SAFETY: Only messages for our private observer are removed.
            if unsafe {
                PeekMessageW(
                    &mut message,
                    self.observer,
                    WM_CLIPBOARDUPDATE,
                    WM_CLIPBOARDUPDATE,
                    PM_REMOVE,
                )
            } != 0
            {
                return true;
            }
            thread::sleep(Duration::from_millis(2));
        }
        false
    }
}

impl Drop for Windows {
    fn drop(&mut self) {
        // SAFETY: Handles are owned by this thread and private station. No user
        // clipboard is touched by destroying these fixture windows.
        unsafe {
            if !self.observer.is_null() {
                RemoveClipboardFormatListener(self.observer);
                DestroyWindow(self.observer);
            }
            if !self.owner.is_null() {
                DestroyWindow(self.owner);
            }
            UnregisterClassW(self.class.as_ptr(), GetModuleHandleW(ptr::null()));
        }
    }
}

fn publish_formats(hwnd: HWND, formats: &[(u32, Vec<u8>)]) -> Result<(), String> {
    let prepared = formats
        .iter()
        .map(|(format, bytes)| PreparedMemory::new(bytes).map(|memory| (*format, memory)))
        .collect::<Result<Vec<_>, _>>()?;
    let _guard = open(hwnd)?;
    // SAFETY: The station and live owner HWND were created by this test.
    if unsafe { EmptyClipboard() } == 0 {
        return Err(native_error("Empty private clipboard"));
    }
    for (format, memory) in prepared {
        memory.transfer(format)?;
    }
    Ok(())
}

fn text_bytes(text: &str) -> Vec<u8> {
    wide(text)
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect()
}

#[test]
#[ignore = "Requires administrator token and dedicated process/private station; run filtered with --test-threads=1"]
fn isolated_win32_clipboard_roundtrips_formats_exclusions_and_events() -> Result<(), String> {
    let _station = PrivateStation::create()?;
    let windows = Windows::create()?;
    let settings = ClipboardSettings {
        monitoring: true,
        ..ClipboardSettings::default()
    };
    let text = "Zone: Tiếng Việt — đường dẫn, ý tưởng 🧭";
    let sequence = write_clipboard(windows.owner, &CopyData::Text(text.into()))?;
    assert_ne!(sequence, 0);
    assert!(
        windows.receives_update(),
        "No native WM_CLIPBOARDUPDATE received"
    );
    let (read_sequence, captured) = read_clipboard(windows.owner, &settings)?;
    assert_eq!(sequence, read_sequence);
    assert!(matches!(captured, Some(Captured::Text(value)) if value == text));

    let paths = vec![
        r"C:\Zone test\đề án.txt".into(),
        r"C:\Zone test\second.txt".into(),
    ];
    write_clipboard(windows.owner, &CopyData::Files(paths.clone()))?;
    assert!(
        matches!(read_clipboard(windows.owner, &settings)?.1, Some(Captured::Files(value)) if value == paths)
    );

    let image = ClipboardImage {
        width: 2,
        height: 1,
        rgba: vec![255, 0, 0, 255, 0, 0, 255, 128],
    };
    let png = image_data::encode_png(&image)?;
    write_clipboard(windows.owner, &CopyData::Image(png.clone()))?;
    {
        let _guard = open(windows.owner)?;
        let cloud_upload = memory(registered(CLOUD_UPLOAD_FORMAT)?, 64)?
            .ok_or("Zone did not publish the Windows cloud clipboard opt-out")?;
        assert_eq!(cloud_upload.get(..4), Some([0; 4].as_slice()));
    }
    let Some(Captured::Image(actual)) = read_clipboard(windows.owner, &settings)?.1 else {
        return Err("PNG/DIB image roundtrip failed".into());
    };
    assert_eq!(
        (actual.width, actual.height, actual.rgba),
        (image.width, image.height, image.rgba.clone())
    );

    let dib = image_data::encode_dib_v5(&image)?;
    publish_formats(windows.owner, &[(CF_DIBV5, dib.clone())])?;
    assert!(
        matches!(read_clipboard(windows.owner, &settings)?.1, Some(Captured::Image(actual)) if actual.rgba == image.rgba)
    );
    publish_formats(
        windows.owner,
        &[
            (registered("PNG")?, b"invalid PNG".to_vec()),
            (CF_DIBV5, dib),
        ],
    )?;
    assert!(
        matches!(read_clipboard(windows.owner, &settings)?.1, Some(Captured::Image(actual)) if actual.rgba == image.rgba)
    );

    for name in [
        "ExcludeClipboardContentFromMonitorProcessing",
        "Clipboard Viewer Ignore",
        "com.agilebits.onepassword",
        "org.nspasteboard.ConcealedType",
        "org.nspasteboard.TransientType",
    ] {
        publish_formats(
            windows.owner,
            &[
                (CF_UNICODETEXT, text_bytes(text)),
                (registered(name)?, vec![1]),
            ],
        )?;
        assert!(
            read_clipboard(windows.owner, &settings)?.1.is_none(),
            "Did not exclude {name}"
        );
    }
    for value in [vec![0, 0, 0, 0], vec![1]] {
        publish_formats(
            windows.owner,
            &[
                (CF_UNICODETEXT, text_bytes(text)),
                (registered("CanIncludeInClipboardHistory")?, value),
            ],
        )?;
        assert!(read_clipboard(windows.owner, &settings)?.1.is_none());
    }
    publish_formats(
        windows.owner,
        &[
            (CF_UNICODETEXT, text_bytes(text)),
            (
                registered("CanIncludeInClipboardHistory")?,
                vec![1, 0, 0, 0],
            ),
        ],
    )?;
    assert!(matches!(
        read_clipboard(windows.owner, &settings)?.1,
        Some(Captured::Text(_))
    ));

    let disabled = ClipboardSettings {
        capture_files: false,
        capture_images: false,
        ..settings
    };
    publish_formats(
        windows.owner,
        &[(CF_HDROP, vec![0; 20]), (CF_UNICODETEXT, text_bytes(text))],
    )?;
    assert!(read_clipboard(windows.owner, &disabled)?.1.is_none());
    publish_formats(
        windows.owner,
        &[
            (registered("PNG")?, png),
            (CF_UNICODETEXT, text_bytes(text)),
        ],
    )?;
    assert!(read_clipboard(windows.owner, &disabled)?.1.is_none());
    // GlobalAlloc may round up and zero allocator padding. Fill the whole
    // allocation so a missing terminator cannot be hidden by incidental padding.
    let malformed = PreparedMemory::new(&[65, 0])?;
    // SAFETY: This is our still-owned movable allocation, before transfer.
    unsafe {
        let size = GlobalSize(malformed.0);
        assert!((2..=4096).contains(&size));
        let data = GlobalLock(malformed.0);
        if data.is_null() {
            return Err(native_error("Lock malformed text fixture"));
        }
        ptr::write_bytes(data.cast::<u8>(), 0x41, size);
        GlobalUnlock(malformed.0);
    }
    {
        let _guard = open(windows.owner)?;
        // SAFETY: The verified private-station window owns this clipboard.
        if unsafe { EmptyClipboard() } == 0 {
            return Err(native_error("Empty private clipboard"));
        }
        malformed.transfer(CF_UNICODETEXT)?;
    }
    assert!(
        read_clipboard(windows.owner, &disabled).is_err(),
        "Unterminated text must be rejected"
    );
    Ok(())
}
