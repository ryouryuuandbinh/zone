# Zone

Một Dynamic Island desktop dành cho Windows 10/11 x64, xây bằng **Tauri 2 + Rust + Microsoft Edge WebView2**, với giao diện React/TypeScript. Zone có nhận diện riêng và không sử dụng mã nguồn hoặc tài nguyên độc quyền của Ciiboard.

Zone có nền tảng **Phase 0/1**, các tiện ích **Phase 2** và công cụ chụp màn hình đầu tiên của **Phase 3**. Nền Dark dùng đen `#000000`. Các kiểm thử Windows chưa chạy được ghi **Not Verified** trong [nhật ký kiểm thử](docs/testing.md).

## Chạy trên Windows

Cần Node.js ≥22.12, npm, Rust stable ≥1.90 dùng target `x86_64-pc-windows-msvc`, Microsoft C++ Build Tools/Visual Studio với **Desktop development with C++**, Windows SDK và Microsoft Edge WebView2 Runtime. Xem [prerequisites chính thức của Tauri](https://v2.tauri.app/start/prerequisites/#windows).

Tại thư mục repository:

```powershell
npm ci
npm run desktop
```

`npm run desktop` chạy `tauri dev`, tự khởi động Vite và build backend Rust. Lần đầu cần Internet để tải dependencies. Cửa sổ Island dùng WebView2 và xuất hiện phía trên desktop; không cần mở trình duyệt để dùng ứng dụng.

Để tạo executable debug tự chứa frontend, không cần giữ Vite chạy:

```powershell
npm run desktop:build:debug
.\src-tauri\target\debug\zone.exe
```

Bản debug này đã được launch và thao tác native trong môi trường hiện tại. Không di chuyển riêng một `zone.exe` được tạo bằng `cargo build` khi frontend vẫn cấu hình dev server: hãy dùng lệnh Tauri ở trên để đóng gói frontend đúng cách.

```powershell
npm run desktop:build
```

Lệnh trên tạo release executable và installer theo cấu hình Tauri. Mã nguồn hiện cấu hình NSIS; việc đóng gói thành công và khả năng cài đặt vẫn phải đối chiếu với [nhật ký kiểm thử](docs/testing.md).

Để chỉ tạo release executable, không tạo installer:

```powershell
npm run desktop:build -- --no-bundle
.\src-tauri\target\release\zone.exe
```

Thoát phiên Zone đang chạy trước khi mở executable mới để kiểm tra đúng bản vừa build. Khi một phiên còn chạy, single-instance chỉ mở lại phiên đó; build thành công không có nghĩa ứng dụng đang mở đã được cập nhật.

Bản build mặc định chưa có chữ ký nhà phát hành. Nếu Windows 11 báo Smart App Control chặn `zone.exe`, xem [cách ký và xác minh bản Windows](docs/windows-signing.md).

Các bản release Phase 2 đã chạy trên Windows. Các lượt kiểm thử clipboard đã xác minh văn bản, khôi phục mục ghim, hình thu nhỏ, sao chép ảnh và dán hai tệp Unicode vào Explorer. Phạm vi kiểm thử của trình chỉnh sửa ảnh, FPS, installer và ma trận cấu hình Windows được ghi riêng trong [nhật ký kiểm thử](docs/testing.md).

`npm run dev` chỉ chạy frontend để phát triển giao diện. Luồng này không có Windows backend và không được dùng làm bằng chứng cho click-through, tray, persistence native hoặc DPI.

## Sử dụng Island

- Rê chuột để phản hồi; tùy chọn mở khi hover được điều chỉnh trong Settings.
- Click Island để mở bảng điều khiển; Escape khi Island đang nhận focus hoặc nút thu gọn để đóng.
- Có thể giữ bảng mở bằng thao tác ghim.
- Phím tắt mặc định: **Ctrl + Shift + Space** mở/thu gọn, **Ctrl + Shift + V** mở clipboard, **Ctrl + Shift + N** mở ghi chú, **Ctrl + Shift + B** mở bookmark. Có thể đổi trong Cài đặt → Phím tắt; lưu sẽ kiểm tra trùng và lỗi đăng ký với Windows.
- Mở Settings từ Island hoặc menu tray. Thông báo thử nghiệm được ghi rõ mục đích, không giả lập sự kiện hệ thống.
- Thoát hẳn bằng **Thoát Zone** trong Settings hoặc menu tray; đóng Settings không đồng nghĩa thoát ứng dụng nền.

Preferences gồm theme Dark/Light/System, accent, kích thước và bo góc, mức animation, hover, auto-collapse, always-on-top, notification timeout, reduced motion, shortcut và startup tùy chọn. Dữ liệu được lưu cục bộ bằng Tauri Store; không có dịch vụ upload hoặc telemetry trong MVP.

## Productivity — Phase 2

- **Clipboard:** văn bản, ảnh bitmap Windows và danh sách đường dẫn tệp; tìm kiếm không dấu, ghim, sao chép lại, xóa từng mục hoặc dọn lịch sử. Theo dõi **tắt mặc định**; bật tại bảng Clipboard hoặc Cài đặt → Tính năng. Bật theo dõi chỉ thu thập các lần sao chép tiếp theo. Có tạm dừng, giới hạn 20–500 mục và tùy chọn thu ảnh/tệp.
- **Chụp màn hình:** mở **Chụp ảnh** → **Chụp màn hình**. Kéo chọn vùng theo hướng dẫn ở giữa phía trên hoặc bấm `Esc` để hủy. Thanh công cụ xuất hiện sau khi chọn vùng, cùng kích thước tính bằng pixel ở góc trên trái. Bạn có thể chú thích, sửa vùng, hoàn tác và làm lại. Dùng **Sao chép** (`Enter` / `Ctrl+C`) hoặc **Lưu ảnh** (`Ctrl+S`) để hoàn tất. Hủy hộp thoại lưu sẽ đưa bạn trở lại vùng đang chỉnh sửa.
- **Ghi chú:** nội dung, việc cần làm, đánh dấu hoàn tất, ghim, sửa, xóa và tìm kiếm. Bấm **Lưu ghi chú** để ghi vào kho native. Bản nháp và phần việc đang gõ được giữ cục bộ khi thu gọn hoặc khởi động lại.
- **Bookmark:** chọn tệp/thư mục bằng hộp thoại Windows hoặc kéo thả, đặt tên, nhóm màu, tìm kiếm và mở vị trí bằng ứng dụng mặc định. Xóa bookmark chỉ xóa mục tham chiếu trong Zone.

Clipboard chưa ghim chỉ nằm trong bộ nhớ phiên theo mặc định; mục đã ghim được lưu. Bật “Lưu lịch sử qua lần mở lại” mới lưu toàn bộ lịch sử. Ảnh gốc được cache riêng trên máy; UI chỉ tải preview tối đa 256 px. Giới hạn tổng payload văn bản/đường dẫn là 16 MiB, cache ảnh là 128 MiB; không tự xóa mục ghim để nhường chỗ. Khi thoát bình thường, cache ảnh chưa ghim được dọn nếu không bật lưu lịch sử. Sau crash, phần cache thừa được dọn ở lần mở tiếp theo.

Dữ liệu ở `%APPDATA%\com.zone.desktop\` (`preferences.json`, `productivity.json`, `clipboard.json`), ảnh ở `%LOCALAPPDATA%\com.zone.desktop\clipboard-cache\`. Bản nháp dùng localStorage của WebView2. Dữ liệu cục bộ chưa mã hóa; không upload. Clipboard tôn trọng các định dạng loại trừ/che giấu khi ứng dụng nguồn cung cấp, không thể tự nhận diện mọi mật khẩu hoặc dữ liệu nhạy cảm. Không bật theo dõi nếu không muốn lưu nội dung đã sao chép.

## Kiểm tra source

```powershell
npm run check
npm run format:check
npm run native:check
npm run native:test
```

`check` chạy TypeScript, ESLint, unit tests và production frontend build. Rust được kiểm tra riêng bằng hai lệnh native. Build thành công không xác minh chất lượng animation hoặc hành vi Windows; kết quả thật và các mục **Not Verified** được ghi ở [docs/testing.md](docs/testing.md).

## Cấu trúc

```text
src/
  app/                 # Application entry and window routing
  animations/          # Shared Motion presets
  components/          # Island, Settings and shared controls
  features/            # Clipboard, notes, bookmarks and shared UI
  services/            # Typed Tauri bridge
  stores/              # State machine and preferences
  styles/              # Theme and shared styles
  types/               # Frontend contracts
src-tauri/
  capabilities/        # Least-privilege Tauri capabilities
  src/                 # Native commands, services, Windows integration
tests/                 # Behavior-focused automated tests
docs/                  # Architecture, verification and roadmap
```

## Tài liệu

- [Kiến trúc và lựa chọn native click-through](docs/architecture.md)
- [Nhật ký và ma trận kiểm thử](docs/testing.md)
- [Cách đo CPU, RAM, startup và frame performance](docs/performance.md)
- [Roadmap từng giai đoạn](docs/roadmap.md)
- [Nguồn tài liệu chính thức](docs/official-references.md)

Phase 1 chỉ đạt nghiệm thu khi launch và tương tác native đã được kiểm chứng, vùng ngoài Island không cản desktop, tray/persistence hoạt động và shutdown sạch. Những kiểm thử DPI, multi-monitor, fullscreen, Explorer restart hoặc chạy dài chưa thực hiện phải tiếp tục được ghi là **Not Verified**.
