//! Windows clipboard ownership and event listener. No clipboard reads happen
//! before monitoring is enabled. A message-only window owns writes/listening.

use super::{
    image_data::{self, ClipboardImage},
    ClipboardItem, ClipboardSettings, Inner, MAX_FILES, MAX_TEXT_BYTES,
};
use std::{
    ptr,
    sync::{
        atomic::{AtomicBool, AtomicU8, Ordering},
        mpsc, Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};
use windows_sys::Win32::{
    Foundation::{GlobalFree, HANDLE, HGLOBAL, HWND},
    System::{DataExchange::*, LibraryLoader::GetModuleHandleW, Memory::*},
    UI::{Shell::DragQueryFileW, WindowsAndMessaging::*},
};

const CF_UNICODETEXT: u32 = 13;
const CF_HDROP: u32 = 15;
const CF_DIB: u32 = 8;
const CF_DIBV5: u32 = 17;
const CLOUD_UPLOAD_FORMAT: &str = "CanUploadToCloudClipboard";
const WAKE: u32 = WM_APP + 0x271;
const MAX_RAW_BYTES: usize = 128 * 1024 * 1024;

pub(super) enum Captured {
    Text(String),
    Files(Vec<String>),
    Image(ClipboardImage),
}

pub(super) enum CopyData {
    Text(String),
    Files(Vec<String>),
    Image(Vec<u8>),
}

enum Request {
    Copy(CopyData, mpsc::SyncSender<Result<(), String>>, CopyPermit),
    Stop,
}

const COPY_PENDING: u8 = 0;
const COPY_COMMITTING: u8 = 1;
const COPY_CANCELLED: u8 = 2;
const COPY_RESULT_UNCERTAIN: &str =
    "Windows đang hoàn tất sao chép; chưa nhận được kết quả. Hãy đợi trước khi thử lại.";

struct CopyControl(AtomicU8);

impl CopyControl {
    fn cancel_pending(&self) -> bool {
        self.0
            .compare_exchange(
                COPY_PENDING,
                COPY_CANCELLED,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
            || self.0.load(Ordering::Acquire) == COPY_CANCELLED
    }

    fn check_pending(&self) -> Result<(), String> {
        if self.0.load(Ordering::Acquire) == COPY_PENDING {
            Ok(())
        } else {
            Err("Thao tác sao chép đã được hủy.".into())
        }
    }

    fn begin_commit(&self) -> Result<(), String> {
        self.0
            .compare_exchange(
                COPY_PENDING,
                COPY_COMMITTING,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .map(|_| ())
            .map_err(|_| "Thao tác sao chép đã được hủy.".into())
    }
}

#[derive(Default)]
struct CopyGate(Arc<AtomicBool>);

impl CopyGate {
    fn reserve(&self) -> Result<CopyPermit, String> {
        self.0
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| {
                "Zone đang sao chép một mục. Vui lòng đợi thao tác đó kết thúc.".to_string()
            })?;
        Ok(CopyPermit {
            gate: Arc::clone(&self.0),
            control: Arc::new(CopyControl(AtomicU8::new(COPY_PENDING))),
        })
    }
}

pub(super) struct CopyPermit {
    gate: Arc<AtomicBool>,
    control: Arc<CopyControl>,
}

impl Drop for CopyPermit {
    fn drop(&mut self) {
        // The worker/request owns this lease, even after the caller times out.
        // At most one large image buffer can wait in the clipboard channel.
        self.gate.store(false, Ordering::Release);
    }
}

fn cancel_or_uncertain(control: &CopyControl, cancelled_error: String) -> String {
    if control.cancel_pending() {
        cancelled_error
    } else {
        // A previously queued WAKE may already have started the synchronous
        // commit even when posting this request's own wake message failed.
        COPY_RESULT_UNCERTAIN.into()
    }
}

fn drain_requests(
    requests: &mpsc::Receiver<Request>,
    stopped: &AtomicBool,
    mut copy: impl FnMut(&CopyData, &CopyControl) -> Result<(), String>,
) -> bool {
    while let Ok(request) = requests.try_recv() {
        match request {
            Request::Stop => return true,
            Request::Copy(data, reply, permit) => {
                if stopped.load(Ordering::Acquire) {
                    permit.control.cancel_pending();
                }
                let result = permit
                    .control
                    .check_pending()
                    .and_then(|()| copy(&data, &permit.control));
                let _ = reply.send(result);
                drop(data);
                drop(permit);
            }
        }
    }
    false
}

pub(super) struct Listener {
    thread_id: u32,
    sender: mpsc::Sender<Request>,
    stopped: Arc<AtomicBool>,
    copy_gate: CopyGate,
    worker: Mutex<Option<JoinHandle<()>>>,
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

impl Listener {
    pub fn start(inner: Arc<Inner>) -> Result<Self, String> {
        let (sender, requests) = mpsc::channel();
        let (ready, receive) = mpsc::sync_channel(1);
        let stopped = Arc::new(AtomicBool::new(false));
        let worker_stopped = Arc::clone(&stopped);
        let worker = thread::Builder::new()
            .name("zone-clipboard".into())
            .spawn(move || {
                // SAFETY: The class and message-only window belong exclusively to
                // this thread. Native callbacks are DefWindowProc only; data travels
                // through Rust channels instead of unchecked callback pointers.
                unsafe {
                    let module = GetModuleHandleW(ptr::null());
                    let class_name = wide("ZoneClipboardListener");
                    let class = WNDCLASSW {
                        lpfnWndProc: Some(DefWindowProcW),
                        hInstance: module,
                        lpszClassName: class_name.as_ptr(),
                        ..Default::default()
                    };
                    if RegisterClassW(&class) == 0 {
                        let _ = ready.send(Err(
                            "Không thể đăng ký cửa sổ theo dõi clipboard.".to_string()
                        ));
                        return;
                    }
                    let hwnd = CreateWindowExW(
                        0,
                        class_name.as_ptr(),
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
                    if hwnd.is_null() {
                        UnregisterClassW(class_name.as_ptr(), module);
                        let _ =
                            ready.send(Err("Không thể tạo cửa sổ theo dõi clipboard.".to_string()));
                        return;
                    }
                    if AddClipboardFormatListener(hwnd) == 0 {
                        DestroyWindow(hwnd);
                        UnregisterClassW(class_name.as_ptr(), module);
                        let _ = ready
                            .send(Err("Windows chưa cho phép theo dõi clipboard.".to_string()));
                        return;
                    }
                    let mut message = MSG::default();
                    PeekMessageW(&mut message, ptr::null_mut(), 0, 0, PM_NOREMOVE);
                    let thread_id = windows_sys::Win32::System::Threading::GetCurrentThreadId();
                    if ready.send(Ok(thread_id)).is_ok() {
                        // Repository I/O stays off the GUI thread. Sequence is
                        // baselined after load, before processing queued updates.
                        inner.initialize();
                        loop {
                            let status = GetMessageW(&mut message, ptr::null_mut(), 0, 0);
                            if status <= 0 {
                                break;
                            }
                            if message.message == WM_CLIPBOARDUPDATE {
                                // Coalesce burst notifications; GetClipboardData reads
                                // the current sequence once, never historical payloads.
                                let mut duplicate = MSG::default();
                                while PeekMessageW(
                                    &mut duplicate,
                                    hwnd,
                                    WM_CLIPBOARDUPDATE,
                                    WM_CLIPBOARDUPDATE,
                                    PM_REMOVE,
                                ) != 0
                                {}
                                inner.capture(hwnd);
                            } else if message.message != WAKE {
                                TranslateMessage(&message);
                                DispatchMessageW(&message);
                            }
                            // If posting WAKE failed because the native queue was
                            // full, an existing message still wakes this loop.
                            // Always drain before blocking again, so cancelled
                            // requests release their permit and Stop is honored.
                            if drain_requests(&requests, &worker_stopped, |data, control| {
                                write_clipboard_controlled(hwnd, data, Some(control))
                                    .map(|sequence| inner.suppress_sequence(sequence))
                            }) {
                                break;
                            }
                        }
                    }
                    RemoveClipboardFormatListener(hwnd);
                    DestroyWindow(hwnd);
                    UnregisterClassW(class_name.as_ptr(), module);
                }
            })
            .map_err(|error| format!("Không thể tạo luồng clipboard: {error}"))?;
        match receive.recv_timeout(Duration::from_secs(3)) {
            Ok(Ok(thread_id)) => Ok(Self {
                thread_id,
                sender,
                stopped,
                copy_gate: CopyGate::default(),
                worker: Mutex::new(Some(worker)),
            }),
            Ok(Err(error)) => {
                let _ = worker.join();
                Err(error)
            }
            Err(error) => Err(format!("Clipboard chưa sẵn sàng: {error}")),
        }
    }

    fn send(&self, request: Request) -> Result<(), String> {
        if self.stopped.load(Ordering::Acquire) {
            return Err("Theo dõi clipboard đã dừng.".into());
        }
        self.sender
            .send(request)
            .map_err(|_| "Không thể gửi thao tác clipboard.")?;
        // SAFETY: The ID is published only after the thread creates its queue.
        if unsafe { PostThreadMessageW(self.thread_id, WAKE, 0, 0) } == 0 {
            return Err("Không thể đánh thức clipboard.".into());
        }
        Ok(())
    }

    pub fn begin_copy(&self) -> Result<CopyPermit, String> {
        if self.stopped.load(Ordering::Acquire) {
            return Err("Clipboard đang dừng.".into());
        }
        self.copy_gate.reserve()
    }

    pub fn copy_with_permit(&self, permit: CopyPermit, data: CopyData) -> Result<(), String> {
        let control = Arc::clone(&permit.control);
        let (reply, result) = mpsc::sync_channel(1);
        if let Err(error) = self.send(Request::Copy(data, reply, permit)) {
            return Err(cancel_or_uncertain(&control, error));
        }
        match result.recv_timeout(Duration::from_secs(4)) {
            Ok(result) => result,
            Err(_) => Err(cancel_or_uncertain(&control, "Sao chép mất quá nhiều thời gian và đã được hủy. Hãy thử lại khi clipboard sẵn sàng.".into())),
        }
    }

    pub fn request_stop(&self) {
        if !self.stopped.swap(true, Ordering::AcqRel) {
            let _ = self.sender.send(Request::Stop);
            // SAFETY: Wake only our own worker thread, which drains Stop and
            // releases the format listener/window before returning.
            unsafe {
                PostThreadMessageW(self.thread_id, WAKE, 0, 0);
            }
        }
    }

    pub fn stop(&self) {
        self.request_stop();
        if let Ok(mut worker) = self.worker.lock() {
            if let Some(handle) = worker.take() {
                let _ = handle.join();
            }
        }
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        self.request_stop();
    }
}

struct ClipboardGuard;
impl Drop for ClipboardGuard {
    fn drop(&mut self) {
        // SAFETY: Created only after successful OpenClipboard on this thread.
        unsafe {
            CloseClipboard();
        }
    }
}

fn open(hwnd: HWND) -> Result<ClipboardGuard, String> {
    for attempt in 0..5 {
        // SAFETY: hwnd is our live listener window; the guard closes this lock.
        if unsafe { OpenClipboard(hwnd) } != 0 {
            return Ok(ClipboardGuard);
        }
        if attempt < 4 {
            thread::sleep(Duration::from_millis(10));
        }
    }
    Err("Clipboard đang được ứng dụng khác sử dụng. Vui lòng thử lại.".into())
}

fn registered(name: &str) -> Result<u32, String> {
    // SAFETY: name is a null-terminated fixed application format name.
    let format = unsafe { RegisterClipboardFormatW(wide(name).as_ptr()) };
    if format == 0 {
        Err("Không thể kiểm tra định dạng clipboard một cách an toàn.".into())
    } else {
        Ok(format)
    }
}

fn memory(format: u32, maximum: usize) -> Result<Option<Vec<u8>>, String> {
    // SAFETY: Caller holds ClipboardGuard. Only global-memory clipboard formats
    // are passed here. GlobalSize bounds every read and the lock is balanced.
    unsafe {
        if IsClipboardFormatAvailable(format) == 0 {
            return Ok(None);
        }
        let handle = GetClipboardData(format);
        if handle.is_null() {
            return Err("Không thể đọc dữ liệu clipboard.".into());
        }
        let size = GlobalSize(handle as HGLOBAL);
        if size == 0 {
            return Ok(Some(Vec::new()));
        }
        if size > maximum {
            return Err("Dữ liệu clipboard vượt giới hạn an toàn.".into());
        }
        let data = GlobalLock(handle as HGLOBAL);
        if data.is_null() {
            return Err("Không thể khóa dữ liệu clipboard.".into());
        }
        let bytes = std::slice::from_raw_parts(data.cast::<u8>(), size).to_vec();
        GlobalUnlock(handle as HGLOBAL);
        Ok(Some(bytes))
    }
}

fn excluded() -> Result<bool, String> {
    for name in [
        "ExcludeClipboardContentFromMonitorProcessing",
        "Clipboard Viewer Ignore",
        "com.agilebits.onepassword",
        "org.nspasteboard.ConcealedType",
        "org.nspasteboard.TransientType",
    ] {
        // SAFETY: Format checks do not read payloads and caller holds clipboard.
        if unsafe { IsClipboardFormatAvailable(registered(name)?) } != 0 {
            return Ok(true);
        }
    }
    if let Some(value) = memory(registered("CanIncludeInClipboardHistory")?, 64)? {
        return Ok(value.len() < 4 || value[..4] == [0, 0, 0, 0]);
    }
    Ok(false)
}

pub(super) fn read_clipboard(
    hwnd: HWND,
    settings: &ClipboardSettings,
) -> Result<(u32, Option<Captured>), String> {
    let _guard = open(hwnd)?;
    // SAFETY: Sequence read is thread safe and identifies this locked snapshot.
    let sequence = unsafe { GetClipboardSequenceNumber() };
    if excluded()? {
        return Ok((sequence, None));
    }
    // Opting out of a content type also excludes its alternate text payload;
    // otherwise a copied file path can bypass the disabled-files preference.
    // SAFETY: Caller owns ClipboardGuard and these checks do not read payloads.
    if !settings.capture_files && unsafe { IsClipboardFormatAvailable(CF_HDROP) } != 0 {
        return Ok((sequence, None));
    }
    if settings.capture_files {
        // SAFETY: HDROP belongs to the clipboard and remains valid under guard.
        unsafe {
            if IsClipboardFormatAvailable(CF_HDROP) != 0 {
                let drop = GetClipboardData(CF_HDROP);
                if drop.is_null() {
                    return Err("Không thể đọc danh sách tệp clipboard.".into());
                }
                let count = DragQueryFileW(drop, u32::MAX, ptr::null_mut(), 0) as usize;
                if count == 0 || count > MAX_FILES {
                    return Err("Clipboard chỉ hỗ trợ tối đa 128 tệp mỗi mục.".into());
                }
                let mut paths = Vec::with_capacity(count);
                for index in 0..count {
                    let length = DragQueryFileW(drop, index as u32, ptr::null_mut(), 0);
                    if length == 0 || length > 32767 {
                        return Err("Đường dẫn clipboard không hợp lệ.".into());
                    }
                    let mut path = vec![0; length as usize + 1];
                    let copied =
                        DragQueryFileW(drop, index as u32, path.as_mut_ptr(), path.len() as u32);
                    if copied != length {
                        return Err("Danh sách tệp clipboard đã thay đổi.".into());
                    }
                    paths.push(
                        String::from_utf16(&path[..length as usize])
                            .map_err(|_| "Tên tệp clipboard không phải Unicode hợp lệ.")?,
                    );
                }
                super::validate_files(&paths)?;
                return Ok((sequence, Some(Captured::Files(paths))));
            }
        }
    }
    let png_format = registered("PNG")?;
    // SAFETY: The format IDs are fixed/registered, checked under ClipboardGuard.
    let has_image = unsafe {
        IsClipboardFormatAvailable(png_format) != 0
            || IsClipboardFormatAvailable(CF_DIBV5) != 0
            || IsClipboardFormatAvailable(CF_DIB) != 0
    };
    if has_image && !settings.capture_images {
        return Ok((sequence, None));
    }
    if settings.capture_images {
        if let Some(image) = read_image(png_format, memory)? {
            return Ok((sequence, Some(Captured::Image(image))));
        }
    }
    if let Some(bytes) = memory(CF_UNICODETEXT, MAX_TEXT_BYTES * 2 + 2)? {
        let text = decode_unicode_text(&bytes)?;
        if !text.trim().is_empty() {
            return Ok((sequence, Some(Captured::Text(text))));
        }
    }
    Ok((sequence, None))
}

fn read_image(
    png_format: u32,
    mut read: impl FnMut(u32, usize) -> Result<Option<Vec<u8>>, String>,
) -> Result<Option<ClipboardImage>, String> {
    let mut first_error = None;
    for (format, limit) in [
        (png_format, image_data::MAX_ENCODED_BYTES),
        (CF_DIBV5, MAX_RAW_BYTES),
        (CF_DIB, MAX_RAW_BYTES),
    ] {
        let decoded = match read(format, limit) {
            Ok(Some(bytes)) if format == png_format => image_data::decode_png(&bytes),
            Ok(Some(bytes)) => image_data::decode_dib(&bytes),
            Ok(None) => continue,
            Err(error) => Err(error),
        };
        match decoded {
            Ok(image) => return Ok(Some(image)),
            Err(error) => {
                first_error.get_or_insert(error);
            }
        }
    }
    // A recognized image that failed to decode must not become alternate text.
    first_error.map_or(Ok(None), Err)
}

fn decode_unicode_text(bytes: &[u8]) -> Result<String, String> {
    let words = bytes.as_chunks::<2>().0;
    let end = words
        .iter()
        .position(|pair| *pair == [0, 0])
        .ok_or("Văn bản clipboard thiếu ký tự kết thúc.")?;
    let words = words[..end]
        .iter()
        .map(|pair| u16::from_le_bytes(*pair))
        .collect::<Vec<_>>();
    let text =
        String::from_utf16(&words).map_err(|_| "Văn bản clipboard không phải Unicode hợp lệ.")?;
    super::validate_text(&text)?;
    Ok(text)
}

struct PreparedMemory(HGLOBAL);

impl PreparedMemory {
    fn new(bytes: &[u8]) -> Result<Self, String> {
        // SAFETY: Own the movable allocation until a successful transfer. Every
        // lock is balanced and allocation happens before EmptyClipboard.
        unsafe {
            let allocation = GlobalAlloc(GMEM_MOVEABLE, bytes.len());
            if allocation.is_null() {
                return Err("Không đủ bộ nhớ cho clipboard.".into());
            }
            let data = GlobalLock(allocation);
            if data.is_null() {
                GlobalFree(allocation);
                return Err("Không thể chuẩn bị clipboard.".into());
            }
            ptr::copy_nonoverlapping(bytes.as_ptr(), data.cast::<u8>(), bytes.len());
            GlobalUnlock(allocation);
            Ok(Self(allocation))
        }
    }

    fn transfer(mut self, format: u32) -> Result<(), String> {
        // SAFETY: Caller holds ClipboardGuard. Windows takes ownership only when
        // SetClipboardData succeeds; Drop frees any untransferred allocation.
        if unsafe { SetClipboardData(format, self.0 as HANDLE) }.is_null() {
            return Err("Không thể ghi dữ liệu vào clipboard.".into());
        }
        self.0 = ptr::null_mut();
        Ok(())
    }
}

impl Drop for PreparedMemory {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: This allocation has not been transferred to Windows.
            unsafe {
                GlobalFree(self.0);
            }
        }
    }
}

fn encode_file_drop(paths: &[String]) -> Result<Vec<u8>, String> {
    super::validate_files(paths)?;
    const HEADER_BYTES: u32 = 20;
    let mut bytes = vec![0; HEADER_BYTES as usize];
    bytes[..4].copy_from_slice(&HEADER_BYTES.to_le_bytes());
    bytes[16..].copy_from_slice(&1u32.to_le_bytes()); // DROPFILES.fWide
    for path in paths {
        for word in wide(path) {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
    }
    bytes.extend_from_slice(&[0, 0]);
    Ok(bytes)
}

#[cfg(test)]
fn write_clipboard(hwnd: HWND, data: &CopyData) -> Result<u32, String> {
    write_clipboard_controlled(hwnd, data, None)
}

fn write_clipboard_controlled(
    hwnd: HWND,
    data: &CopyData,
    control: Option<&CopyControl>,
) -> Result<u32, String> {
    if let Some(control) = control {
        control.check_pending()?;
    }
    // Prepare/validate all buffers before replacing the user's current clipboard.
    let buffers: Vec<(u32, Vec<u8>)> = match data {
        CopyData::Text(text) => {
            super::validate_text(text)?;
            vec![(
                CF_UNICODETEXT,
                wide(text)
                    .iter()
                    .flat_map(|word| word.to_le_bytes())
                    .collect(),
            )]
        }
        CopyData::Files(paths) => vec![(CF_HDROP, encode_file_drop(paths)?)],
        CopyData::Image(bytes) => {
            let image = image_data::decode_png(bytes)?;
            vec![
                (CF_DIBV5, image_data::encode_dib_v5(&image)?),
                (registered("PNG")?, bytes.clone()),
            ]
        }
    };
    let mut prepared = Vec::with_capacity(buffers.len() + 1);
    // Set the Windows cloud opt-out before any user payload. A failed marker
    // transfer must not publish data that Windows may synchronize elsewhere.
    prepared.push((
        registered(CLOUD_UPLOAD_FORMAT)?,
        PreparedMemory::new(&0u32.to_le_bytes())?,
    ));
    for (format, bytes) in buffers {
        prepared.push((format, PreparedMemory::new(&bytes)?));
    }
    let _guard = open(hwnd)?;
    // Cancellation and the start of commit arbitrate atomically. A timed-out
    // queued request cannot change the clipboard later. No rollback is claimed
    // after the synchronous Win32 commit boundary has been entered.
    if let Some(control) = control {
        control.begin_commit()?;
    }
    // SAFETY: The owner is a non-null live message-only window opened above.
    if unsafe { EmptyClipboard() } == 0 {
        return Err("Không thể thay nội dung clipboard.".into());
    }
    for (format, memory) in prepared {
        memory.transfer(format)?;
    }
    // Read the sequence while still locked; a later external copy must not be
    // suppressed as if it belonged to Zone.
    Ok(unsafe { GetClipboardSequenceNumber() })
}

pub(super) fn copy_data(item: &ClipboardItem) -> Result<CopyData, String> {
    match item.kind.as_str() {
        "text" => Ok(CopyData::Text(
            item.text.clone().ok_or("Mục văn bản không hợp lệ.")?,
        )),
        "files" => Ok(CopyData::Files(item.files.clone())),
        _ => Err("Loại clipboard chưa được hỗ trợ.".into()),
    }
}

pub(super) fn current_sequence() -> u32 {
    // SAFETY: This reads only the public clipboard sequence counter, not data.
    unsafe { GetClipboardSequenceNumber() }
}

#[cfg(test)]
mod file_drop_tests {
    use super::*;

    #[test]
    fn windows_shell_decodes_two_unicode_paths_from_owned_file_drop() {
        let paths = [
            r"C:\Zone thử nghiệm\Đề án.txt".to_owned(),
            r"C:\Zone thử nghiệm\Tệp thứ hai 🏝.txt".to_owned(),
        ];
        let payload = encode_file_drop(&paths).unwrap();
        let memory = PreparedMemory::new(&payload).unwrap();
        // SAFETY: The owned movable allocation contains DROPFILES and a UTF-16
        // list. It remains live throughout the calls and is never transferred.
        unsafe {
            assert_eq!(
                DragQueryFileW(memory.0, u32::MAX, ptr::null_mut(), 0),
                paths.len() as u32
            );
            for (index, expected) in paths.iter().enumerate() {
                let index = index as u32;
                let expected: Vec<_> = expected.encode_utf16().chain([0]).collect();
                let length = DragQueryFileW(memory.0, index, ptr::null_mut(), 0);
                assert_eq!(length as usize + 1, expected.len());
                let mut actual = vec![u16::MAX; expected.len()];
                let copied =
                    DragQueryFileW(memory.0, index, actual.as_mut_ptr(), actual.len() as u32);
                assert_eq!(copied, length);
                assert_eq!(actual, expected);
            }
        }
    }
}

#[cfg(test)]
mod image_tests {
    use super::*;

    const PNG_FORMAT: u32 = 0xc000;

    #[test]
    fn malformed_png_uses_valid_dibv5_pixels() {
        let image = ClipboardImage {
            width: 2,
            height: 1,
            rgba: vec![255, 0, 0, 255, 0, 0, 255, 128],
        };
        let dib = image_data::encode_dib_v5(&image).unwrap();
        let actual = read_image(PNG_FORMAT, |format, _| match format {
            PNG_FORMAT => Ok(Some(b"invalid PNG".to_vec())),
            CF_DIBV5 => Ok(Some(dib.clone())),
            _ => panic!("A valid DIBV5 should end the search"),
        })
        .unwrap()
        .unwrap();
        assert_eq!(
            (actual.width, actual.height, actual.rgba),
            (image.width, image.height, image.rgba)
        );
    }

    #[test]
    fn failed_image_reads_are_distinct_from_absent_images() {
        let mut tried = Vec::new();
        let result = read_image(PNG_FORMAT, |format, limit| {
            tried.push((format, limit));
            match format {
                PNG_FORMAT => Err("first image format failed to render".into()),
                CF_DIBV5 => Ok(Some(vec![0; 8])),
                _ => Ok(None),
            }
        });
        assert_eq!(
            result.err().as_deref(),
            Some("first image format failed to render")
        );
        assert_eq!(
            tried,
            [
                (PNG_FORMAT, image_data::MAX_ENCODED_BYTES),
                (CF_DIBV5, MAX_RAW_BYTES),
                (CF_DIB, MAX_RAW_BYTES),
            ]
        );
        assert!(read_image(PNG_FORMAT, |_, _| Ok(None)).unwrap().is_none());
    }
}

#[cfg(test)]
mod text_tests {
    use super::*;

    #[test]
    fn unicode_text_requires_a_terminator_and_valid_surrogate_pairs() {
        assert!(decode_unicode_text(&[b'a', 0]).is_err());
        assert!(decode_unicode_text(&[0, 0xd8, 0, 0]).is_err());
        assert!(decode_unicode_text(&[0]).is_err());
        let bytes: Vec<_> = wide("Tiếng Việt 🏝")
            .iter()
            .flat_map(|word| word.to_le_bytes())
            .collect();
        assert_eq!(decode_unicode_text(&bytes).unwrap(), "Tiếng Việt 🏝");
    }

    #[test]
    fn allocator_padding_after_the_terminator_is_not_captured() {
        assert_eq!(decode_unicode_text(&[b'a', 0, 0, 0, 0xff]).unwrap(), "a");
        assert_eq!(decode_unicode_text(&[0, 0]).unwrap(), "");
    }
}

#[cfg(test)]
mod copy_tests {
    use super::*;

    #[test]
    fn cancelled_request_drained_without_its_own_wake_releases_the_gate() {
        let gate = CopyGate::default();
        let permit = gate.reserve().unwrap();
        let control = Arc::clone(&permit.control);
        let (send, receive) = mpsc::channel();
        let (reply, result) = mpsc::sync_channel(1);
        send.send(Request::Copy(
            CopyData::Text("fixture".into()),
            reply,
            permit,
        ))
        .unwrap();

        // Simulate a failed PostThreadMessage after enqueue, not a Win32 call.
        assert_eq!(
            cancel_or_uncertain(&control, "wake failed".into()),
            "wake failed"
        );
        assert!(gate.reserve().is_err());
        let stopped = AtomicBool::new(false);
        assert!(!drain_requests(&receive, &stopped, |_, _| {
            panic!("a cancelled request must never prepare or commit clipboard data")
        }));
        assert!(result.try_recv().unwrap().is_err());
        assert!(gate.reserve().is_ok());
    }

    #[test]
    fn failed_wake_reports_uncertainty_when_an_earlier_wake_started_commit() {
        let gate = CopyGate::default();
        let worker_owned = gate.reserve().unwrap();
        assert!(worker_owned.control.begin_commit().is_ok());
        assert_eq!(
            cancel_or_uncertain(&worker_owned.control, "wake failed".into()),
            COPY_RESULT_UNCERTAIN,
        );
        assert!(gate.reserve().is_err());
        drop(worker_owned);
        assert!(gate.reserve().is_ok());
    }

    #[test]
    fn queued_stop_cancels_pending_copy_without_requiring_a_wake_message() {
        let gate = CopyGate::default();
        let (send, receive) = mpsc::channel();
        let (reply, result) = mpsc::sync_channel(1);
        send.send(Request::Copy(
            CopyData::Text("fixture".into()),
            reply,
            gate.reserve().unwrap(),
        ))
        .unwrap();
        send.send(Request::Stop).unwrap();
        let stopped = AtomicBool::new(true);
        assert!(drain_requests(&receive, &stopped, |_, _| {
            panic!("shutdown must reject a copy still pending in the channel")
        }));
        assert!(result.try_recv().unwrap().is_err());
        assert!(gate.reserve().is_ok());
    }

    #[test]
    fn timeout_does_not_release_a_request_still_owned_by_the_worker() {
        let gate = CopyGate::default();
        let permit = gate.reserve().unwrap();
        let control = Arc::clone(&permit.control);
        let (send, receive) = mpsc::channel();
        send.send(permit).unwrap();
        assert!(control.cancel_pending());
        assert!(gate.reserve().is_err());
        let queued = receive.recv().unwrap();
        assert!(queued.control.begin_commit().is_err());
        drop(queued);
        assert!(gate.reserve().is_ok());
    }

    #[test]
    fn cancellation_and_commit_have_one_winner() {
        let gate = CopyGate::default();
        let cancelled = gate.reserve().unwrap();
        assert!(cancelled.control.cancel_pending());
        assert!(cancelled.control.begin_commit().is_err());
        drop(cancelled);
        let writing = gate.reserve().unwrap();
        assert!(writing.control.begin_commit().is_ok());
        // A caller must report uncertainty once native commit has begun.
        assert!(!writing.control.cancel_pending());
        assert!(gate.reserve().is_err());
        drop(writing);
        assert!(gate.reserve().is_ok());
    }

    #[test]
    fn failed_preparation_or_dropped_request_releases_its_reservation() {
        let gate = CopyGate::default();
        let preparing = gate.reserve().unwrap();
        assert!(gate.reserve().is_err());
        drop(preparing);
        let queued = gate.reserve().unwrap();
        let (send, receive) = mpsc::channel();
        send.send(queued).unwrap();
        drop(receive);
        assert!(gate.reserve().is_ok());
    }

    #[test]
    fn simultaneous_timeout_and_commit_cannot_both_succeed() {
        for _ in 0..32 {
            let control = Arc::new(CopyControl(AtomicU8::new(COPY_PENDING)));
            let barrier = Arc::new(std::sync::Barrier::new(2));
            let worker_control = Arc::clone(&control);
            let worker_barrier = Arc::clone(&barrier);
            let worker = thread::spawn(move || {
                worker_barrier.wait();
                worker_control.begin_commit().is_ok()
            });
            barrier.wait();
            let cancelled = control.cancel_pending();
            let committed = worker.join().unwrap();
            assert_ne!(cancelled, committed);
        }
    }
}

#[cfg(test)]
#[path = "live_tests.rs"]
mod live_tests;
