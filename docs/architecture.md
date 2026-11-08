# Kiến trúc Zone

Zone là ứng dụng Windows x64 dùng Tauri 2, Rust và Microsoft Edge WebView2. React chỉ vẽ giao diện trong cửa sổ native; đây không phải trang web mô phỏng desktop.

## Phạm vi nền tảng

- **React + TypeScript strict:** giao diện, truy cập bàn phím, điều phối tương tác, animation.
- **Motion for React:** morphing, spring, shared elements, enter/exit và reduced motion.
- **Zustand:** trạng thái Island và preferences trong bộ nhớ của mỗi cửa sổ.
- **Rust:** tạo cửa sổ, tray, vị trí màn hình, hit region Windows, lifecycle và xác thực lệnh.
- **Tauri Store:** preferences và cache kho dữ liệu cục bộ. Không có backend Internet hoặc telemetry.
- **Tailwind CSS + Lucide React:** design tokens, layout và icon. Nhận diện Zone được tạo riêng.

Settings dùng cửa sổ riêng. Island khởi động ẩn và chỉ hiển thị sau khi frontend sẵn sàng, để hạn chế nền trắng hoặc nội dung chưa được dựng.

### Luồng tạo WebView trên Windows

Tạo Settings từ command bất đồng bộ và `spawn_blocking`, không gọi `WebviewWindowBuilder::build()` trực tiếp trong synchronous IPC hoặc native event callback. Tauri nêu rõ nguy cơ deadlock WebView2 trong các ngữ cảnh này. Tray cũng gọi cùng async service. Việc tạo được serialize bằng mutex chỉ giữ trên blocking pool để tránh hai yêu cầu nhanh tạo trùng label; GUI thread không chờ mutex đó.

### Dependency snapshot

Lockfiles hiện resolve Tauri Rust 2.12.5; API/CLI 2.12.4; React 19.3.1; Motion 14.9.1; Vite 8.3.6; Tailwind 4.3.4; Zustand 5.0.16; Lucide React 1.69.0; TypeScript 6.0.3. Native plugins: Store 2.5.0, Autostart 2.7.0, Global Shortcut 2.4.1, Dialog 2.8.4, Opener 2.7.2, Single Instance 2.5.5. TypeScript 6 được giữ để tương thích peer range của typescript-eslint 8.72.1. Rust minimum khai báo 1.90; toolchain thực tế dùng kiểm tra là 1.99.1.

## Quyết định cửa sổ và click-through

### Các phương án đã cân nhắc

| Phương án                                | Ưu điểm                                                               | Đánh đổi                                                                                                          |
| ---------------------------------------- | --------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------- |
| Resize HWND theo từng frame              | Bounds native bám giao diện                                           | Resize WebView2 liên tục dễ gây redraw, flicker và chi phí layout                                                 |
| Canvas trong suốt + CSS `pointer-events` | Dễ viết UI                                                            | CSS không quyết định hit testing giữa các cửa sổ Windows                                                          |
| `WM_NCHITTEST` trả `HTTRANSPARENT`       | Có thể hit-test theo tọa độ                                           | Microsoft chỉ mô tả chuyển tiếp tới cửa sổ phía dưới **cùng thread**; không đủ cho click xuyên sang ứng dụng khác |
| Canvas cố định + region native           | Giữ kích thước WebView2 ổn định; ngoài region không thuộc vùng cửa sổ | Cần đồng bộ hình học giữa renderer và Rust; region có thể cắt bóng hoặc nội dung khi hai timeline lệch nhau       |

### Phương án Phase 1

Island nằm ở giữa phía trên canvas native trong suốt 720 × 560 logical px, neo vào work area màn hình chính. Bản hiện tại dùng **native-pointer-hit-test**: giữ `WS_EX_LAYERED`, bật `WS_EX_TRANSPARENT` khi cursor nằm ngoài rounded rectangle đang hiển thị. Không còn dùng `SetWindowRgn`: cạnh clip pixel của region từng làm mất khử răng cưa của WebView2.

Frontend gửi mục tiêu hình học khi trạng thái đổi, không gửi IPC từng frame. Rust lấy mẫu spring tương ứng để hit-test. `WH_MOUSE_LL` observer trên thread riêng chỉ đọc tọa độ con trỏ, không ghi log, nuốt hoặc tạo input. Thay đổi style chạy trên GUI thread qua bounded message 2 ms; khi quá hạn, gửi yêu cầu tính lại từ cursor hiện tại, không áp dụng trạng thái cũ. `WM_STYLECHANGED` khôi phục các bit cần thiết nếu Tao thay đổi style.

Rust và Motion vẫn là hai timeline riêng; cùng preset không bảo đảm cùng timestamp/subpixel. Cần kiểm thử cạnh bo góc, đảo chiều nhanh và cursor đứng yên trong lúc morph. Worker chỉ chạy khi có chuyển động; lúc idle chặn trên channel. HWND/WebView2 không resize từng frame. Không coi CSS `pointer-events` là giải pháp native click-through.

Observer unhook và kết thúc message loop khi dừng. Mọi Win32/`unsafe` nằm trong module Windows riêng, có giải thích hiệu lực HWND, tọa độ, ownership và cleanup. Shutdown bình thường dừng observer rồi join clipboard trên blocking pool trong lúc GUI còn xử lý được thông điệp Windows; sau đó đóng kho dữ liệu và thoát. Fallback `RunEvent::Exit` chỉ yêu cầu clipboard dừng, không join trên GUI thread.

## Hình học và trình tự animation

Surface chính animate trực tiếp ba giá trị số `width`, `height`, `borderRadius` bằng Motion spring cùng stiffness, damping và mass với Rust. Không dùng FLIP scale cho toàn bộ Island hoặc shared transform cho brand bên trong: scale lồng nhau trước đây làm hình học hiển thị lệch native region và biến dạng nội dung. `layout`, `layoutId` và shared transition vẫn dùng ở khu vực tab phù hợp.

Đổi width/height trong DOM có chi phí layout; đây là đánh đổi có chủ đích trên surface nhỏ để giữ hình dạng bo góc và text ổn định. HWND/WebView2 vẫn giữ canvas cố định. Cần profile chi phí dựng frame trước khi kết luận đạt 60/120 FPS.

Mỗi layer compact/expanded/notification trong `AnimatePresence` giữ kích thước riêng cho đến khi exit kết thúc. Vì vậy, footer của panel cũ không bị bố trí lại theo kích thước pill khi thu gọn. Layer outgoing không nhận pointer/keyboard input. Nội dung expanded vào sau 140 ms, layer cũ fade out 90 ms; reduced motion bỏ các độ trễ và chuyển động này.

Geometry worker chờ 8 ms chỉ trong transition và yêu cầu observer cập nhật hit eligibility theo spring hiện tại, không repaint hoặc resize native window. Observer giới hạn yêu cầu GUI bằng bounded message. Một RAII guard gọi `timeBeginPeriod(1)` trong thời gian có chuyển động và ghép `timeEndPeriod(1)` khi settle, worker kết thúc hoặc guard bị drop. Khi idle, worker chặn trên channel, không polling và không giữ yêu cầu timer precision. Đây là chính sách lên lịch, không phải bằng chứng về frame rate hiển thị.

## Điều phối sự kiện

Tương tác trực tiếp có ưu tiên cao hơn thông báo thụ động. Khi người dùng đang mở một panel, thông báo được xếp hàng hoặc hiển thị trong panel thay vì thay nội dung đang thao tác. Thu gọn không được hủy queue hoặc làm mất preferences.

Chỉ lớp điều phối Island quyết định trạng thái hiện tại; module chức năng tương lai phát sự kiện theo hợp đồng chung. Không để từng module tự resize cửa sổ, tự giành focus hoặc ghi đè giao diện.

State machine có `idle`, `hovered`, `expanded`, `notification`; panel `overview`, `clipboard`, `notes`, `bookmarks`, `activity`. Queue tối đa 8 thông báo, lịch sử phiên tối đa 30, loại trùng theo ID. Hover giữ 220 ms mới mở; auto-collapse mặc định 650 ms. Interaction locks giữ panel khi soạn ghi chú, xác nhận hoặc mở native picker. `OPEN_PANEL` không phá queue; pending notifications được giữ đến khi tương tác kết thúc.

Timeout hover, auto-collapse và notification phải bị hủy khi điều kiện không còn đúng hoặc component unmount. Tauri event listeners được hủy khi cửa sổ đóng/unmount; xử lý cả trường hợp lời hứa đăng ký listener hoàn thành sau cleanup.

## DPI, màn hình và focus

Tách tọa độ logic của UI khỏi pixel vật lý của Windows. Hit testing sử dụng scale factor hiện hành; tọa độ màn hình có thể âm ở cấu hình nhiều màn hình. Khi DPI hoặc bố cục màn hình đổi, tính lại vị trí top-center và hit geometry.

Island không chủ động giành focus khi chỉ hover hoặc có thông báo. Click trực tiếp, phím tắt hoặc tray có thể yêu cầu focus bằng `focus_island`; native bỏ `WS_EX_NOACTIVATE` cho tương tác rõ ràng và phục hồi chế độ thụ động khi mất focus. Khi thu gọn, `release_island_focus` cố trả focus về cửa sổ trước đó sau khi kiểm tra HWND còn hợp lệ, và chỉ làm vậy nếu Island vẫn đang là foreground window. Settings là cửa sổ tương tác riêng có thể được focus theo hành động người dùng. Không coi `always-on-top` là cam kết hiển thị trên mọi ứng dụng fullscreen độc quyền.

Bản hiện tại chỉ neo vào màn hình chính, chưa có bộ chọn màn hình. Có xử lý sự kiện DPI, `WM_DISPLAYCHANGE` và `WM_SETTINGCHANGE`, nhưng thay màn hình chính/mixed-DPI/hot-plug vẫn cần kiểm thử thực tế.

## Hợp đồng native

| Command                                  | Trách nhiệm                                                                 |
| ---------------------------------------- | --------------------------------------------------------------------------- |
| `get_preferences` / `update_preferences` | Đọc, validate, áp dụng và lưu preferences bằng Rust + Tauri Store           |
| `set_island_layout`                      | Nhận mục tiêu `width`, `height`, `radius`, `animate` theo đơn vị logic      |
| `island_ready`                           | Áp dụng hit testing hiện tại trước khi show; ghi mốc frontend-ready lần đầu |
| `focus_island`                           | Nhận focus theo thao tác rõ ràng của người dùng                             |
| `release_island_focus`                   | Trở lại chế độ thụ động và trả focus có điều kiện khi thu gọn               |
| `show_settings`                          | Tạo hoặc hiển thị cửa sổ `index.html?window=settings`                       |
| `send_notification`                      | Validate nội dung và gửi sự kiện thông báo tới Island                       |
| `get_runtime_info`                       | Trả phiên bản, scale, monitor, shortcut/startup và diagnostics đang có      |
| `exit_app`                               | Đánh dấu thoát có chủ đích và kết thúc ứng dụng                             |

Sự kiện frontend nghe: `zone://toggle`, `zone://open-panel`, `zone://notification`, `zone://preferences`, `zone://clipboard-changed`, `zone://productivity-changed`. Listener luôn cleanup, kể cả đăng ký hoàn tất sau unmount. Chỉ Rust truy cập Store; preferences dùng `preferences.json` với key `preferences`.

## Phase 2 — Productivity

`features/clipboard` dùng message-only HWND và `AddClipboardFormatListener`, không polling. Monitoring tắt mặc định; sequence được baseline ở startup/bật lại để không thu nội dung clipboard cũ. Đọc các format exclusion và dữ liệu trong cùng một OpenClipboard guard; không suy đoán mọi chuỗi mật khẩu. Unicode text, DIB/DIBV5 và CF_HDROP đều kiểm tra kích thước. Copy lại chạy qua listener sở hữu clipboard, không giữ capture mutex khi chờ worker. UI chỉ nhận thumbnail PNG tối đa 256 px; ảnh gốc riêng phục vụ copy.

Zone thử PNG, DIBV5 rồi DIB khi clipboard có nhiều bản của cùng ảnh. Nếu tất cả bản đọc được đều lỗi, Zone báo lỗi ảnh. Khi chép lại văn bản, ảnh hoặc tệp, Zone ghi cờ `CanUploadToCloudClipboard=0` trước nội dung để yêu cầu Windows giữ mục trên thiết bị.

History chưa ghim ở RAM mặc định. Các mục đã ghim luôn được lưu, còn lưu toàn bộ lịch sử là tùy chọn. Tổng text/path tối đa 16 MiB, image cache 128 MiB, ảnh tối đa 16 MP/32 MiB PNG, 128 đường dẫn/mục, history 20–500. Prune chỉ mục chưa ghim và tệp tạm có tên do Zone tạo, không xóa đệ quy. Ảnh cache thiếu giữ lại metadata/pin để người dùng xử lý.

Cache dùng tên sinh từ ID đã kiểm tra, là tệp trực tiếp trong `APPLOCALDATA/clipboard-cache`. Rust mở tệp Windows mà không theo reparse point, kiểm tra metadata trên cùng handle và giới hạn số byte đọc. Cách này hỗ trợ thư mục AppData mà Windows chuyển hướng tệp vào vùng dữ liệu riêng của gói ứng dụng. Giao thức Tauri `zone-thumbnail` chỉ phục vụ GET từ cửa sổ Island theo ID mục ảnh hiện có và trả PNG tối đa 256 px mỗi chiều. Snapshot không cấp đường dẫn tệp cho WebView.

Tắt thu ảnh/tệp chặn toàn bộ mục có định dạng tương ứng, kể cả khi nguồn cung cấp thêm biểu diễn text. UTF16 phải có terminator và surrogate hợp lệ; độ dài đường dẫn giới hạn theo UTF16 units của Windows. BI_RGB32 không dùng byte dự trữ làm alpha; chỉ mask alpha được khai báo mới tạo transparency. Frontend serialize refresh qua mutation: snapshot cũ không thể hoàn tác pause/delete và các lần đọc trong lúc ghi được gộp để chạy sau phản hồi ghi.

Khi Windows tạo DIBV5 từ DIB, bộ giải mã nhận diện trường hợp ba mặt nạ RGB bị lặp sau header bằng cả kích thước dư và nội dung byte. Các DIBV5 hợp lệ giữ nguyên vị trí pixel.

Copy có giới hạn một tác vụ trên cả frontend và native. Native giữ permit từ trước khi đọc ảnh đến khi worker xử lý hoặc bỏ request; caller timeout không giải phóng slot sớm. Trạng thái Pending/Committing/Cancelled phân xử bằng atomic CAS ngay trước EmptyClipboard: timeout thắng khi còn Pending thì request không thể ghi muộn. Nếu Windows đã bắt đầu commit đồng bộ, UI báo chưa nhận kết quả, không hứa rollback. Việc theo dõi clipboard và nút tạm dừng vẫn độc lập với tác vụ copy.

Repository có đường mở theo thư mục rõ ràng để kiểm thử chính luồng disk/lease/atomic write trong thư mục tạm, không cần AppHandle hay clipboard hệ thống. Khi chạy app, cùng đường mở được dùng rồi gắn private Store cache đã detach.

`features/productivity` quản lý notes/todos, bookmarks và categories. CRUD clone candidate, validate, ghi thành công rồi mới publish; lỗi I/O giữ dữ liệu cũ. Giới hạn 1.000 notes, 500 bookmarks, 50 categories, tổng JSON 8 MiB. Bookmark native picker không block GUI; chỉ mở ID đã lưu sau khi canonicalize và xác minh loại tệp/thư mục. Không nhận URI, device path hoặc NTFS alternate streams. Xóa bookmark không xóa tệp đích, xóa category chỉ bỏ nhóm. Frontend draft có validation và được giữ trong localStorage đến Save/Discard.

Hai kho dùng private Tauri Store cache đã detach khỏi resource table: plugin Store có thể save mọi registered store khi Exit dù tắt autosave. Metadata được ghi temp/sync/atomic rename trước khi cập nhật cache, có file lease chống phiên cũ ghi đè. Tệp hỏng được báo lỗi và giữ nguyên. Single-instance plugin được đăng ký đầu tiên; phiên thứ hai chỉ yêu cầu mở panel, không xử lý args như lệnh.

Bốn shortcut được chuẩn hóa, kiểm tra trùng nội bộ và thử đăng ký Windows trước khi bỏ binding cũ. Nếu lưu thất bại, rollback đăng ký/startup/topmost/cache. Writer mutex riêng tránh giữ preferences read lock trong lúc gọi API hệ thống; quit chờ writer đang chạy xong. Không đăng ký global shortcut chỉ bằng một phím chữ không modifier.

Startup diagnostic đo từ lúc vào hàm Rust `run()` đến `island_ready`; không gồm launcher/khởi tạo process của Windows và không tự chứng minh thời điểm pixel đầu tiên hiện trên màn hình. CPU, RAM và FPS trả `null` khi chưa có profiler; UI phải giữ nhãn chưa đo.

## Phase 3 — Chụp màn hình

Zone lấy kích thước và vị trí vật lý của màn hình chứa Island, tạm ẩn các cửa sổ Zone đang hiển thị rồi dùng GDI `BitBlt` trên worker. `DwmFlush` chờ các cập nhật trình bày của ứng dụng. Phiên chụp giữ các cửa sổ đó ẩn đến khi editor đóng hoặc gặp lỗi. Các lối mở Island qua khay hệ thống, phím tắt và khởi tạo WebView tôn trọng trạng thái này. GDI DC và bitmap được giải phóng khi worker rời hàm. Ảnh là RGB 8 bit, giới hạn 16 megapixel và 32 MiB PNG.

Cửa sổ chỉnh sửa phủ đúng hình chữ nhật vật lý của màn hình đã chụp và hiện sau khi WebView vẽ khung đầu tiên. Hướng dẫn kéo chọn hiện ở giữa phía trên. Thanh công cụ chỉ hiện khi đã có vùng chọn. Tài liệu giữ tọa độ pixel của ảnh gốc, vùng xuất độc lập với các nét vẽ và lịch sử thay đổi có giới hạn. Nhờ đó, các lớp giữ vị trí khi di chuyển hoặc thay đổi kích thước vùng.

Màn hình xem trước dùng canvas có giới hạn độ phân giải. Khi xuất, trình ghép vẽ ảnh nguồn và các lớp chú thích ở độ phân giải gốc rồi cắt đúng hình chữ nhật đã chọn. Cut Out nối các dải còn lại và có thể làm kích thước tệp nhỏ hơn vùng chụp. Khung vùng, số đo và thanh công cụ nằm ngoài canvas xuất. Rust kiểm tra định dạng, kích thước và phiên trước khi ghi clipboard hoặc mở hộp thoại lưu. Clipboard dùng hàng đợi Windows và cờ chống đồng bộ. Lưu chỉ tạo tệp mới theo đường dẫn người dùng chọn.

Zone giữ một phiên chỉnh sửa đang mở và tối đa bốn ảnh ghim. Cửa sổ ghim chỉ được đọc đúng ảnh của nó và dữ liệu được giải phóng khi đóng. Ảnh chèn PNG, JPEG hoặc WebP được đọc từ tệp người dùng chọn trong WebView. Giao thức ảnh nguồn và ảnh ghim không cấp đường dẫn tệp cho WebView.

## Dữ liệu, IPC và bảo mật

- Lệnh nhận số phải kiểm tra hữu hạn, khoảng cho phép và giới hạn kích thước.
- Preferences phải được validate tại Rust trước khi áp dụng hoặc lưu; dữ liệu file cũ/hỏng cần xử lý rõ ràng.
- Chỉ cấp capability cho đúng cửa sổ và API đang dùng; không thêm quyền filesystem hoặc shell rộng cho tính năng chưa có.
- Không đọc clipboard, file của người dùng, nội dung agent, secrets hoặc media nếu module tương ứng chưa được triển khai và bật.
- Luồng preview trong trình duyệt, nếu dùng để kiểm tra UI, phải ghi rõ không có Windows backend. Thành công ở preview không chứng minh native integration hoạt động.

## Ranh giới mở rộng module

Clipboard, ghi chú, dấu trang và chụp màn hình có module riêng. Recorder, transfer, media và agent monitor được bổ sung theo roadmap. Frontend quản lý hiển thị. Rust quản lý tài nguyên native và tác vụ nền.

Chi tiết tình trạng từng giai đoạn ở [roadmap.md](roadmap.md). Kết quả kiểm thử thực tế ở [testing.md](testing.md).
