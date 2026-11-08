# Kiểm thử và điều kiện nghiệm thu

Trạng thái **Not Verified** nghĩa là chưa chạy hoặc chưa có bằng chứng đủ để kết luận. Code review, unit test và browser preview không thay thế kiểm thử overlay Windows. Chưa đánh dấu toàn bộ Phase 1 hoàn tất khi các điều kiện native bắt buộc vẫn chưa được xác minh.

## Môi trường kiểm tra ngày 05/11/2026

- Windows x64; Rust/Cargo 1.99.1, toolchain `stable-x86_64-pc-windows-msvc`.
- Windows registry: DisplayVersion 25H2, build 26200.9711.
- Node.js 26.13.0, npm 11.20.0.
- Visual Studio Community 2022 17.14.44, bộ MSVC x86/x64 được `vswhere` tìm thấy.
- Windows SDK 10.0.26100.0 hiện diện.
- Microsoft Edge WebView2 Runtime 156.0.4314.58 được xác nhận qua registry.

Đây là kiểm tra cài đặt công cụ, chưa phải kết quả build hoặc runtime.

## Kiểm tra tự động — nền tảng ngày 05/11/2026

| Hạng mục                            | Trạng thái   | Bằng chứng                                                                             |
| ----------------------------------- | ------------ | -------------------------------------------------------------------------------------- |
| Cài dependencies/lockfile           | Passed       | npm và Cargo lockfiles đã resolve; build dùng dependency thực                          |
| TypeScript strict                   | Passed       | `npm run check` → `tsc -b --pretty`                                                    |
| ESLint                              | Passed       | `npm run check`: 0 warnings                                                            |
| Frontend production build           | Passed       | Vite: JS 400.19 kB (gzip 127.88 kB), CSS 27.12 kB; 898 ms                              |
| Unit tests state/notification queue | Passed       | Vitest 5.1.0: 10/10, gồm keyboard-open dismissal                                       |
| `cargo check --locked` Windows MSVC | Passed       | Native agent chạy thành công                                                           |
| Rust unit tests                     | Passed       | `cargo test --locked --offline`: 6/6                                                   |
| Debug executable build              | Passed       | `cargo build --locked --offline`                                                       |
| Rust formatting                     | Passed       | `cargo fmt --check`                                                                    |
| Rust Clippy                         | Passed       | `cargo clippy --all-targets -- -D warnings`                                            |
| Measurement script syntax           | Passed       | PowerShell parser, không lấy mẫu ứng dụng khác                                         |
| Tauri development launch            | Passed       | `npm run desktop` đã khởi động Vite, Rust và cửa sổ native                             |
| Tauri embedded debug build + launch | Passed       | `tauri build --debug --no-bundle`; Vite đã dừng; WebView dùng `http://tauri.localhost` |
| Release executable build            | Not Verified | Chưa ghi nhận                                                                          |

Browser UI đã được mở và thao tác bằng CUA: compact, expanded, activity và close có bố cục vừa khung, không thấy nội dung chồng lấn trong các trạng thái đã quan sát. Đây chỉ là kiểm tra frontend; không xác minh tray, region Windows, focus native hoặc persistence.

### Dependency audit

`npm audit --omit=dev` báo **0 vulnerabilities** cho production dependencies. Audit toàn bộ cây còn **10 moderate** trong tooling phát triển, liên quan `minimatch` 10.2.6 qua ESLint/typescript-eslint; upstream advisory [GHSA-g3hv-75wg-m249](https://github.com/advisories/GHSA-g3hv-75wg-m249) nêu bản sửa 11.0.1 ngoài semver hiện tại của dependency chain. Chưa dùng force update, downgrade hoặc override ép phiên bản không tương thích. Cần cập nhật tooling khi upstream hỗ trợ bản vá, rồi chạy lại audit và các kiểm tra build/lint.

## Ma trận Windows thực tế

Các kết quả native dưới đây được quan sát bằng CUA trên bản Tauri debug tự chứa frontend ở DPI 200%. Không suy rộng sang DPI hoặc cấu hình chưa thử.

| Tình huống                               | Bằng chứng / điều kiện còn thiếu                                                                                                                                        | Trạng thái   |
| ---------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------ |
| Launch desktop tự chứa                   | Chạy executable sau khi dừng Vite; AX URL `http://tauri.localhost`                                                                                                      | Passed       |
| Compact / expanded                       | Click pill mở panel, nút đóng thu gọn trên overlay native                                                                                                               | Passed       |
| Ghim panel                               | Pin giữ panel mở trong thao tác đã quan sát                                                                                                                             | Passed       |
| Global shortcut mặc định                 | Ctrl + Shift + Space hoạt động ngoài Island; runtime báo Registered                                                                                                     | Passed       |
| Notification khi đang tương tác          | Thông báo thử được queue, panel giữ nguyên; đóng panel hiện notification và timeout                                                                                     | Passed       |
| Settings mở riêng                        | Bản cuối mở cửa sổ thành công sau sửa deadlock tạo WebView                                                                                                              | Passed       |
| Lưu appearance                           | Chọn Light, Save hiện Preferences saved, Island native đổi sang Light                                                                                                   | Passed       |
| Preferences qua restart                  | Quit, mở lại executable; pill vẫn Light                                                                                                                                 | Passed       |
| Quit từ Settings                         | Các cửa sổ Zone đóng sau Quit Zone; kiểm tra process/tài nguyên còn lại riêng                                                                                           | Passed       |
| DPI 200%                                 | Runtime báo scale 200%; đã thao tác giao diện native ở cấu hình này                                                                                                     | Passed       |
| Startup không flash / flicker            | Chưa có kiểm tra chuyển động hoặc trace đủ để kết luận                                                                                                                  | Not Verified |
| Repeated expand/collapse                 | Chưa chạy đủ ≥30 chu kỳ                                                                                                                                                 | Not Verified |
| Rapid clicks / hover                     | Chưa xác minh stress đảo chiều và clipping                                                                                                                              | Not Verified |
| Click xuyên ngoài compact                | Calculator đổi 0 → 9 sau click thực tại (699, 530) trong canvas 720 × 560 nhưng ngoài pill; Zone vẫn hiện phía trên. Đã thử trên bản embedded trước đợt sửa motion cuối | Passed       |
| Click xuyên ngoài expanded / chuyển động | Lượt thử expanded chưa hoàn tất; chưa xác minh cạnh/góc bo khi morph                                                                                                    | Not Verified |
| Focus / Alt+Tab                          | Cần kiểm tra passive hover, trả focus sau collapse và chuyển ứng dụng                                                                                                   | Not Verified |
| System tray GUI                          | Cần thao tác Show/Hide/Settings/Quit từ tray trực tiếp                                                                                                                  | Not Verified |
| Shortcut conflict                        | Cần thử chord bị ứng dụng khác giữ; không coi Registered mặc định là đã thử conflict                                                                                    | Not Verified |
| Startup with Windows                     | Chưa đăng xuất/đăng nhập để thử bật/tắt                                                                                                                                 | Not Verified |
| Fullscreen                               | Chưa thử video hoặc ứng dụng fullscreen                                                                                                                                 | Not Verified |
| DPI 100% / 125% / 150%                   | Chưa đổi và thử từng cấu hình                                                                                                                                           | Not Verified |
| Multi-monitor                            | Chưa thử đổi primary, hot-plug, tọa độ âm hoặc mixed DPI                                                                                                                | Not Verified |
| Resolution change                        | Chưa đổi độ phân giải trong runtime                                                                                                                                     | Not Verified |
| Windows theme switching                  | Light/Dark trong app không thay thế thử System theme theo Windows                                                                                                       | Not Verified |
| Explorer restart                         | Chưa restart Explorer và xác minh tray phục hồi                                                                                                                         | Not Verified |
| Long run                                 | Chưa chạy soak test ≥2 giờ                                                                                                                                              | Not Verified |
| Failure / crash recovery                 | Chưa thử settings hỏng hoặc phục hồi sau crash                                                                                                                          | Not Verified |
| Shutdown / resource cleanup              | Cần xác nhận không còn process Zone và tài nguyên bất thường sau Quit                                                                                                   | Not Verified |

### Regression đã sửa

Bản đầu bị deadlock khi tạo Settings trong synchronous command. Đã chuyển sang async service + `spawn_blocking` theo cảnh báo chính thức của Tauri và serialize việc tạo cửa sổ. Bản embedded debug mới nhất đã mở Settings, lưu Light và restart thành công. Đây là bằng chứng cho đường tương tác đã chạy; chưa phải stress test hàng loạt yêu cầu mở Settings.

### Điều chỉnh motion sau phản hồi thực tế

Đã thay root FLIP/nested brand transforms bằng numeric width/height/radius spring, giữ geometry riêng cho layer outgoing để tránh footer nhảy vị trí, và điều chỉnh thứ tự fade. Probe Windows cho thấy độ chờ 16 ms mặc định có median thực 31,08 ms; chuyển worker sang chờ 8 ms với timer precision scoped cho median 8,31 ms trong probe. Đã mở bản native mới ở DPI 200%; Rust 6 tests, Clippy `-D warnings` và fmt được chạy lại thành công. Chưa có đo FPS/frame presentation hoặc nghiệm thu đầy đủ cảm giác chuyển động; các kiểm thử stress và gate Phase 1 vẫn chưa hoàn tất.

### Đo nhanh debug

Runtime ghi startup **513 ms** từ Rust `run()` đến frontend-ready, không gồm launcher của Windows. Mẫu idle 10,31 giây gồm 8 process Zone/WebView2, CPU chuẩn hóa theo 18 logical CPU: **0,0% sau làm tròn**, private commit **188,07 MiB**, tổng working set **428,8 MiB**. Xem [artifact gốc](../artifacts/idle-performance.json) và [giới hạn phép đo](performance.md). Đây không phải số đo release hoặc cam kết FPS.

## Definition of done — Phase 1

Frontend và Rust phải build không lỗi; Tauri launch được; Island thật có compact/expanded và Motion; vùng ngoài không cản desktop; tray và preferences hoạt động; shutdown sạch; source modular và có hướng dẫn. Những tiêu chí cần GUI ở trên vẫn phải xác minh trên Windows. Không gán Passed dựa trên việc API tồn tại.

Ghi kết quả mới cùng ngày, cấu hình máy, lệnh/các thao tác đã chạy và bằng chứng. Số đo hiệu năng có mẫu riêng ở [performance.md](performance.md).

## Đợt Phase 2 — 06/11/2026

Bản Windows debug tự chứa frontend được chạy bằng Computer Use skill trên cùng máy, DPI 200%. Các kết quả ngày 05/11 ở trên là lịch sử; bảng dưới đây mô tả bản mới. Nền Dark đã đổi sang `#000000`; canvas ngoài Island vẫn trong suốt. Region clip pixel được thay bằng native cursor hit testing để WebView2 giữ cạnh khử răng cưa. Chưa có phép đo FPS hoặc video đủ để kết luận cảm giác motion đạt nghiệm thu.

### Build và kiểm thử tự động

- `npm run check`: TypeScript strict, ESLint 0 cảnh báo, **18/18 Vitest**, frontend production build thành công.
- Rust: **32/32 unit tests** qua sau khi bổ sung manifest Common Controls v6 cho test harness. Các test gồm spring/hit geometry, migration preferences, shortcut conflicts, notes/todos/categories, file locking, atomic persistence/rollback, giới hạn và projection riêng tư clipboard, DIB/PNG và thumbnail giữ tỉ lệ.
- `cargo test --locked --offline`: toàn bộ lib/bin/doc-test targets qua; 32 tests thực ở lib, bin/doc-test không có test. `cargo clippy --all-targets --locked --offline -- -D warnings`, `cargo fmt --check` và `npm run format:check`: qua. Cargo có cảnh báo không hard-link được incremental cache trong môi trường hạn chế nên copy file; không có lỗi lint source bị bỏ qua.
- `npm run desktop:build:debug`: qua; executable tại `src-tauri/target/debug/zone.exe`. Bundle lần cuối: JS **455,06 kB**, gzip **142,64 kB**; CSS **39,67 kB**, gzip **8,53 kB**.
- Hai test atomic rename bị `Access is denied` trong môi trường hạn chế; cùng bộ test chạy trên Windows host đã qua. Đây không phải lý do bỏ validation hoặc thay atomic write bằng ghi đè không an toàn.
- Lỗi loader `STATUS_ENTRYPOINT_NOT_FOUND` được truy về `TaskDialogIndirect` yêu cầu Common Controls v6. Build dùng một manifest linker chung; tắt manifest RC trùng của Tauri nhưng giữ icon/version resources. Cả test harness và embedded app đã build/launch được.
- Debug app dùng Windows subsystem để không mở thêm console khi chạy executable. Installer/release vẫn **Not Verified**.

### Quan sát native thực tế

| Tình huống                                                | Trạng thái   | Bằng chứng / giới hạn                                                                                                                                                                            |
| --------------------------------------------------------- | ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Nền đen và giao diện tiếng Việt                           | Passed       | Quan sát pill, notes, bookmarks, clipboard và Settings native; nền chính đen, canvas ngoài trong suốt                                                                                            |
| Ghi chú tiếng Việt và todo                                | Passed       | Tạo tiêu đề/nội dung, gõ todo chưa bấm dấu +, Save tự kèm todo; note xuất hiện trong danh sách                                                                                                   |
| Draft qua collapse                                        | Passed       | Thu gọn rồi Ctrl+Shift+N; tiêu đề, body và text todo chưa thêm vẫn còn                                                                                                                           |
| Notes qua restart                                         | Passed       | Thoát, mở lại, note và todo giữ nguyên; toggle hoàn thành được xác nhận `done=true` trong dữ liệu sau thoát                                                                                      |
| Native folder picker                                      | Passed       | Chọn `C:\workspace\dev\zone\docs`; modal hoạt động, panel không tự mất khi chọn                                                                                                                  |
| Bookmark open                                             | Passed       | Mở bookmark; Explorer xuất hiện tại thư mục `docs`                                                                                                                                               |
| Bookmark qua restart                                      | Passed       | Sau Quit/relaunch, bookmark `docs` còn trong danh sách                                                                                                                                           |
| Phím tắt module mặc định                                  | Passed       | Ctrl+Shift+N/B/V mở đúng notes/bookmarks/clipboard trên native                                                                                                                                   |
| Single instance                                           | Passed       | Mở executable lần hai trở về Overview; Windows host xác nhận chỉ một Zone process                                                                                                                |
| Clipboard mặc định                                        | Passed       | Bảng báo đang tạm dừng, 0 mục; không bật monitoring trong đợt GUI QA                                                                                                                             |
| Click xuyên compact                                       | Passed       | Calculator 0→9 tại screen logical (365,345), nằm trong canvas Zone nhưng ngoài pill                                                                                                              |
| Click xuyên expanded                                      | Passed       | Ghim bảng mở; cùng điểm trên Calculator 9→99; quan sát Zone vẫn expanded và pinned                                                                                                               |
| Settings và Quit                                          | Passed       | Settings riêng mở được; nút Thoát đóng cả hai cửa sổ                                                                                                                                             |
| Process cleanup                                           | Passed       | Theo dõi 9 Zone/conhost/WebView2 processes trước Quit; sau Quit không còn PID nào. Xem `artifacts/phase2-shutdown.json`                                                                          |
| Clipboard live text/image/file, copy/pin/delete           | Not Verified | Backend/event listener và tests dữ liệu đã có; chưa bật theo dõi hoặc chạy vòng copy/paste thật                                                                                                  |
| Clipboard sensitivity formats                             | Not Verified | Chưa thử với password manager/app nguồn thật; không cam kết phát hiện mọi dữ liệu nhạy cảm                                                                                                       |
| Notes search/pin/delete, bookmark file/category/drag-drop | Not Verified | Đã có source/backend và unit validation; chưa chạy hết đường GUI                                                                                                                                 |
| Custom shortcut / xung đột app khác                       | Not Verified | Test chuẩn hóa/trùng nội bộ đã qua; chưa thử chord do một app khác giữ                                                                                                                           |
| Cạnh hit test trong morph, hover nhanh, ≥30 chu kỳ        | Not Verified | Hai trạng thái ổn định đã thử; chưa nghiệm thu stress hoặc timing cạnh trong animation                                                                                                           |
| CPU/RAM idle debug 10 giây                                | Passed       | Mẫu 10,363 giây, 7 processes: CPU chuẩn hóa 0,0% sau làm tròn, private commit 276,50 MiB, tổng working set 535,43 MiB; xem `artifacts/phase2-idle-performance.json` và giới hạn ở performance.md |
| FPS, startup mới, long run, fullscreen                    | Not Verified | Chưa đo frame/startup Phase 2 hoặc benchmark release; mẫu idle ngắn không thay thế các mục này                                                                                                   |
| DPI 100/125/150%, multi-monitor, Explorer restart         | Not Verified | Chưa thay đổi cấu hình hệ thống để kiểm tra                                                                                                                                                      |

Ghi chú và bookmark thử được dọn sau khi đóng ứng dụng, chỉ theo đúng ID/nội dung do lượt QA này tạo; không đưa mock data vào production flow. Bằng chứng metadata ở `artifacts/phase2-fixture-cleanup.json`. Theo dõi clipboard giữ nguyên tắt mặc định.

## Rà soát Phase 2 — 06–07/11/2026

Đã sửa snapshot clipboard cũ ghi đè pause/delete, refresh đến giữa lúc ghi làm mất dữ liệu mới, và thao tác Settings trước khi tải cài đặt thật. Refresh được gộp và chờ mutation kết thúc, không dùng polling. Thứ tự mục sao chép lại theo recency backend được giữ đúng.

Native sửa BI_RGB32 coi byte dự trữ như alpha; bổ sung offset bảng màu truecolor, UTF16 terminator/surrogate và giới hạn đường dẫn theo UTF16 units. Tắt thu tệp/ảnh chặn cả biểu diễn text thay thế. Ảnh cache thiếu giữ metadata/pin và báo cảnh báo. Dọn cache nhận diện đúng ảnh tạm do ghi bị ngắt, chỉ xóa tên thuộc Zone.

- `npm run check`: TypeScript, ESLint 0 warnings, **23/23 Vitest**, frontend build qua. Có 5 regression tests cho thứ tự trả về của clipboard IPC.
- `cargo test --locked --offline`: **38 passed, 1 ignored**; bin/doc-test targets không có test. Bài ignored kiểm thử Win32 clipboard ở window station riêng; không tính Passed.
- `cargo clippy --all-targets --locked --offline -- -D warnings` và `cargo fmt --check`: qua. Kiểm tra Prettier qua sau định dạng.
- `npm run desktop:build:debug`: qua, bản mới đã launch và tương tác native. Bundle JS 455,42 kB/gzip 142,82 kB, CSS 39,69 kB/gzip 8,54 kB.
- 30 chu kỳ Ctrl+Shift+Space đã kiểm tra expanded và compact sau mỗi chuyển trạng thái (chờ 650 ms). Tiếp tục 60 toggle nhanh, mỗi action + AX snapshot mất 77–182 ms, kết thúc compact. Đây là thời gian thao tác kiểm thử, **không phải frame time/FPS**.
- Khi soạn bản nháp, 10 toggle nhanh giữ nguyên tiêu đề; sau chuyển focus sang Calculator, editor vẫn mở dù không ghim. Interaction lock được đăng ký lại khi đảo chiều trước unmount và giải phóng khi tab đang exit.

Bằng chứng chức năng: [phase2-interaction-regression.json](../artifacts/phase2-interaction-regression.json). Kiểm thử này không xác minh rapid mouse click/hover, cạnh hit test trong morph, flicker theo frame hoặc FPS.

### Kiểm thử clipboard tách biệt — Not Verified

`live_tests.rs` dùng Win32 production read/write và một station không tương tác với clipboard riêng. `CWF_CREATE_ONLY`, kiểm tra `WSF_VISIBLE` và xác minh station hiện tại bảo đảm không dùng lại WinSta0/clipboard đang có. Không chuyển input desktop, không thay tùy chọn theo dõi clipboard.

Lượt chạy hiện tại bị từ chối tạo station có tên (`ERROR_ACCESS_DENIED`); thử station tự đặt tên với CREATE_ONLY trả `ERROR_ALREADY_EXISTS` nên từ chối dùng lại. Cả hai dừng **trước khi đọc/ghi clipboard**. Windows yêu cầu administrator token cho station mới có tên; chưa có lượt chạy thành công trong môi trường này. Unit tests codec và state không thay thế bài này.

Chỉ trên Windows test runner có quyền tạo station riêng, chạy bài này một mình trong process riêng:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib --locked isolated_win32_clipboard -- --ignored --test-threads=1
```

Không thêm fallback dùng clipboard desktop thật. Các luồng capture/copy/pin/delete qua GUI clipboard tiếp tục **Not Verified**. Không tự bật monitoring hay thay đổi tùy chọn đã lưu của người dùng trong đợt rà soát. Bản nháp thử được lưu rồi dọn đúng ID khi Zone đã thoát; xem [metadata dọn dữ liệu thử](../artifacts/phase2-interaction-fixture-cleanup.json).

## Clipboard storage và copy concurrency — 07/11/2026

Đã tách repository theo đường dẫn để gọi chính disk/lease/atomic-write production code trong thư mục tạm. Chín integration tests mới kiểm tra Unicode/pins qua reopen, opt-out full history, vòng đời ảnh chưa ghim, xóa mục/dọn lịch sử, metadata ảnh thiếu, cache sau crash, tệp hỏng không bị ghi đè, lease giữa hai writer và lỗi atomic replace trên Windows. Không sử dụng clipboard hệ thống, GUI hoặc AppData thật trong các test này.

Giới hạn copy mới có hai lớp: frontend giữ một `copyingId`, native giữ permit từ trước khi chuẩn bị ảnh đến lúc worker hoàn tất hoặc bỏ request. Timeout không mở slot sớm. Pending/Committing/Cancelled dùng atomic CAS để request bị hủy khi còn chờ không thể ghi vào clipboard muộn hơn. Sau khi bắt đầu EmptyClipboard đồng bộ, lỗi timeout báo chưa xác định kết quả thay vì hứa rollback. Stop tiếp tục không block GUI.

- `npm run check`: **27 tests passed**, TypeScript strict, ESLint 0 warnings và frontend build qua. Bốn tests copy mới gồm chống click trùng/khác ID, retry sau lỗi, guard initial load, refresh/pause khi copy đang chờ.
- `cargo test --locked --offline`: **51 passed, 1 ignored**; gồm 9 test repository và 4 test ownership/cancel/commit race mới. Bài live station vẫn ignored và **Not Verified**.
- `cargo clippy --all-targets --locked --offline -- -D warnings`, `cargo fmt --check`, Prettier: qua.
- Kiểm tra quyền Windows hiện tại trả non-administrator; không chạy lại bài station đã biết bị chặn. Cửa sổ Zone được mở để người dùng chủ động opt-in và sao chép fixture `Zone kiểm thử 07-11`; chưa coi là xác minh live khi chưa nhận trạng thái sẵn sàng từ người dùng.
- `npm run desktop:build -- --no-bundle`: **Passed**, release profile optimized build hoàn tất sau 4 phút 13 giây. Executable tại `src-tauri/target/release/zone.exe`; không đóng cửa sổ debug đang chờ bước kiểm thử của người dùng. Build metadata/checksum ở [release-build.json](../artifacts/release-build.json). Release runtime, FPS/CPU/RAM và installer vẫn **Not Verified**; không tính việc single-instance mở lại phiên debug là launch release.

## Clipboard văn bản và release runtime — 07/11/2026

Người dùng đã tự bật monitoring và xác nhận sao chép đúng fixture `Zone kiểm thử 07-11`. Chỉ mục này được thao tác trong lượt thử; agent không đổi monitoring hoặc cài đặt riêng tư.

| Đường tương tác                      | Trạng thái | Bằng chứng                                                                                             |
| ------------------------------------ | ---------- | ------------------------------------------------------------------------------------------------------ |
| Capture Unicode text thật            | Passed     | Bảng clipboard native xuất hiện đúng fixture 19 ký tự sau thao tác sao chép của người dùng             |
| Tìm kiếm có/không dấu                | Passed     | `zone kiem thu 07-11` tìm thấy fixture; query không tồn tại ẩn mục và hiện trạng thái không có kết quả |
| Chép lại và Windows paste trên debug | Passed     | Bấm nội dung mẫu, nhận feedback; Ctrl+V vào ô tìm kiếm trả đúng fixture có dấu, thay query không dấu   |
| Ghim trước Quit                      | Passed     | UI chuyển sang “Bỏ ghim mục clipboard”, xác nhận save thành công trước thoát                           |
| Khôi phục pin khi mở release         | Passed     | Sau Quit debug và launch executable release, đúng fixture vẫn có trạng thái đã ghim                    |
| Release launch thực                  | Passed     | Computer Use trả app path `target/release/zone.exe`; không có phiên debug trong danh sách trước launch |
| Clipboard shortcut trên release      | Passed     | Ctrl+Shift+V mở bảng clipboard của phiên release                                                       |
| Copy/paste trên release mới          | Passed     | Có trạng thái “Đang chép…”, sau đó “Đã chép lại”; Windows Ctrl+V trả đúng fixture                      |
| Không nhân bản self-copy             | Passed     | Lịch sử vẫn một mục sau copy lại trên release                                                          |
| Thu gọn sau lượt chính               | Passed     | Escape đưa release về pill; trạng thái ghim bảng tạm được xóa, pin của mục mẫu được giữ                |

Bằng chứng metadata: [clipboard-live-verification.json](../artifacts/clipboard-live-verification.json). AX snapshot có thể chụp trước React commit; các kết luận dùng lần đọc mới sau khi giao diện cập nhật. Bảng được ghim tạm trong lượt automation để tránh thu gọn do đổi focus; không coi đây là stress test hover/focus đầy đủ.

Copy/paste xác nhận đúng fixture và feedback thành công từ native. Clipboard trước đó đã chứa cùng fixture, nên việc thay một giá trị khác có sẵn chưa được xác minh độc lập. Lượt đối chứng bổ sung dừng khi phát hiện người dùng đang tương tác; chưa sao chép baseline khác. Cửa sổ được bàn giao ở bảng Clipboard đang mở, giữ nguyên lựa chọn theo dõi và mục mẫu đã ghim.

Trong code review đã sửa hai nhánh hiếm của native queue: wake thất bại phải báo kết quả chưa xác định nếu commit đã bắt đầu; worker drain requests sau mỗi native message để cancelled Copy và Stop không bị kẹt khi chính wake của chúng thất bại. **Ba regression tests mới qua**, tổng Rust hiện **54 passed, 1 ignored**. Clippy `-D warnings`, fmt và release build cuối qua. Release cuối build trong 1 phút 44 giây; checksum mới ở [release-build.json](../artifacts/release-build.json).

Đã lấy mẫu release khi pill nghỉ trong 10,227 giây: 7 process Zone/WebView2, CPU chuẩn hóa 0,0% sau làm tròn, private commit 280,26 MiB, tổng working set 534,05 MiB. Đây là mẫu ngắn, không phải benchmark hoặc đo FPS; chi tiết và giới hạn ở [performance.md](performance.md) và [release-idle-performance.json](../artifacts/release-idle-performance.json).

Fixture vẫn được ghim sau kiểm thử. Không chạy GUI delete/clear; **ảnh/tệp clipboard live, sensitivity flags với ứng dụng nguồn, FPS, installer và toàn bộ ma trận Windows còn Not Verified**. Bài private-window-station vẫn ignored; không chuyển kết quả text fixture thành tuyên bố test đó đã qua.

## Clipboard ảnh và tệp — 07/11/2026

- Đã xác minh hồi quy bằng dữ liệu trong bộ nhớ. PNG hỏng kèm DIBV5 hợp lệ trả đúng pixel. Ảnh lỗi ở mọi định dạng trả lỗi thay vì được xử lý như clipboard không có ảnh.
- Khi chép lại, Zone ghi `CanUploadToCloudClipboard=0` trước payload. Đã bổ sung kiểm tra cờ này và trường hợp nhiều định dạng ảnh vào bài Win32 tách biệt. Bài này vẫn **Not Verified** vì phiên hiện tại thiếu quyền tạo window station riêng.
- Bộ test Rust có **56 đạt, 1 ignored**. Clippy với `-D warnings` và `cargo fmt --all --check` đạt.
- `npm run desktop:build:debug` qua và tạo bản Windows tại `src-tauri/target/debug/zone.exe`. Lượt mở giao diện dùng bản release trước đó.
- Bản release cũ mở được bảng Clipboard, nhưng Zone báo đang tắt theo dõi. File Explorer mở được cửa sổ, nhưng quyền thao tác cửa sổ hết thời gian chờ. Chưa sao chép dữ liệu mẫu vào clipboard Windows. Capture, ảnh xem trước và chép/dán ảnh hoặc tệp qua giao diện vẫn **Not Verified**.

Để thử với dữ liệu mẫu, dùng [ảnh 73 × 37 px](../artifacts/clipboard-format-fixtures/Zone-anh-mau-73x37.png) và hai tệp Unicode trong [thư mục Nguồn](../artifacts/clipboard-format-fixtures/Nguồn). Sao chép pixel từ trình sửa ảnh, rồi sao chép hai tệp từ Explorer. Sau khi cả hai mục xuất hiện trong Zone, chép lại ảnh và dán vào trình sửa ảnh. Tiếp đó chép lại danh sách tệp và dán vào [thư mục Kết quả](<../artifacts/clipboard-format-fixtures/Kết quả>). Đối chiếu ảnh và nội dung hai tệp để kiểm tra việc thay thế hai loại dữ liệu qua clipboard Windows.

### Kiểm thử Windows với dữ liệu mẫu

- **Tệp — Passed.** API Windows đưa hai đường dẫn mẫu Unicode vào clipboard. Zone nhận cả hai và ghi lại đúng danh sách khi clipboard đang chứa ảnh mẫu. Explorer dán vào thư mục trống. Hai tệp ở đích có SHA-256 trùng với nguồn, và cả hai tệp nguồn vẫn còn.
- **Ảnh — tái hiện lỗi trên bản release trước bản sửa.** Zone nhận ảnh bitmap 73 × 37 px và lưu hai tệp cache. Hình thu nhỏ không hiện, thao tác chép lại báo ảnh nằm ngoài bộ nhớ của Zone. Probe chỉ đọc metadata xác nhận đường dẫn chuẩn của thư mục nằm dưới AppData logic, còn đường dẫn chuẩn của tệp ảnh nằm dưới `Packages/…/LocalCache/Local`. Đây là hai tên Windows trả cho cùng cây dữ liệu qua cơ chế chuyển hướng AppData.
- Thêm bài thử `windows_shell_decodes_two_unicode_paths_from_owned_file_drop`. Shell Windows đọc đúng hai đường dẫn Unicode từ bộ đệm do Zone cấp phát. Bài này không mở clipboard Windows.
- **Tái hiện lệch pixel sau khi sửa đọc cache.** Bản debug hiển thị hình thu nhỏ và khôi phục ảnh đã ghim, nhưng ảnh cũ chép lại khớp 2.350/2.701 pixel với mẫu gốc ở cả PNG và DIBV5. Cờ chống đồng bộ Windows có giá trị DWORD 0. Mục mẫu cũ đã được bỏ ghim sau khi thử.
- Probe phát đúng `CF_DIB` 32 bit có ba mặt nạ màu. DIBV5 Windows tạo có header 124 byte và vùng nhớ 10.940 byte, dư đúng 12 byte so với bố cục chuẩn. Ba mặt nạ lặp ở offset 124. Đọc pixel từ 124 khớp 2.350/2.701, từ 136 khớp 2.701/2.701. Bản sửa chỉ dùng offset 136 nếu đủ dấu hiệu và giữ nguyên trường hợp DIBV5 hợp lệ.

### Kết quả bản release cuối

| Đường tương tác                                 | Kết quả | Bằng chứng                                                                                                                     |
| ----------------------------------------------- | ------- | ------------------------------------------------------------------------------------------------------------------------------ |
| Capture DIBV5 tổng hợp và hình thu nhỏ          | Passed  | Nạp lại cùng CF_DIB lỗi, Zone hiện ảnh 73 × 37 với đúng hình thu nhỏ                                                           |
| Thay danh sách tệp bằng ảnh từ Zone             | Passed  | Xác nhận clipboard ban đầu chứa đúng hai tệp mẫu. Sau “Chép lại”, bộ đọc bitmap Windows khớp **2.701/2.701 pixel** với PNG gốc |
| Cờ chống đồng bộ trên ảnh chép lại              | Passed  | Win32 đọc `CanUploadToCloudClipboard` là DWORD 0                                                                               |
| Thay ảnh bằng danh sách tệp và dán vào Explorer | Passed  | Explorer dán đủ hai tệp tên tiếng Việt vào thư mục trống. SHA-256 khớp từng tệp nguồn và nguồn vẫn còn                         |
| Thu gọn để bàn giao                             | Passed  | Escape đưa Zone về pill, bản release vẫn chạy                                                                                  |

Frontend **27 đạt**. Rust **60 đạt, 1 ignored**. Clippy, định dạng và build release không installer đều qua. [Thông tin executable](../artifacts/release-build.json). Ảnh được đối chiếu qua bộ đọc clipboard Windows. Dán ảnh vào trình sửa ảnh riêng, bài Win32 ở station tách biệt, FPS, installer và toàn bộ ma trận Windows vẫn **Not Verified**.

## Trình chỉnh sửa ảnh chụp — Phase 3

- Bài kiểm thử mô hình và thao tác kiểm tra Undo/Redo, điểm bút sau khi di chuyển hoặc đổi kích thước, đổi vùng chụp mà giữ chú thích, thứ tự xóa lớp, quy đổi tọa độ khi thay đổi kích thước viewport, và hợp nhất các dải Cut Out.
- `node scripts/Test-ScreenshotEditorRender.mjs` chạy Canvas trong Chrome headless với hồ sơ tạm và ảnh mẫu. Đã kiểm tra pixel đầu của vùng cắt, 42 pixel sau khi ghép hai dải, chú thích trong PNG và kích thước JPEG sau giải mã. Bài thử giao diện mô phỏng kéo chuột để vẽ trước khi chọn vùng, kiểm tra nhãn `120 × 100 px`, chuyển sang hậu kỳ, Undo/Redo vùng chụp và đối chiếu màu nét vẽ trong ảnh xuất.
- Bộ Rust kiểm tra cắt theo pixel, bảo toàn tệp đích đã tồn tại, định dạng/kích thước ảnh xuất và quyền truy cập của cửa sổ ghim.
- **34 bài frontend và 64 bài Rust đạt**, 1 bài Win32 tách biệt bị bỏ qua theo cấu hình. TypeScript, ESLint, Clippy và định dạng đều qua. Build release x64 không installer thành công. [Thông tin bản build](../artifacts/release-build.json).
- **Not Verified trên giao diện Windows:** mở editor từ màn hình đã đóng băng, chuyển chế độ với chú thích đang mở, hộp thoại lưu, clipboard, cửa sổ ghim, kéo chuột trên nhiều màn hình, DPI 100% / 125% / 150%, và hiệu năng khung hình. Lượt kiểm thử giao diện trước đó đã dừng theo thao tác của người dùng.

## Luồng chụp một vùng — 08/11/2026

- Chromium headless đã kiểm tra màn hình Chụp ảnh chỉ có một nút, hướng dẫn giữa phía trên hiện trước khi chọn, Enter trước khi chọn không xuất ảnh, kéo xong hiện thanh công cụ và nhãn `120 × 100 px`. Bài thử xác nhận `Enter` / `Ctrl+C` gọi sao chép, `Ctrl+S` gọi lưu, hủy lưu giữ vùng chọn và sao chép thành công đóng editor. `Ctrl+C` trong ô nhập không gọi xuất ảnh.
- Bài thử vẫn đối chiếu đúng pixel vùng cắt, chú thích sau Undo/Redo, Cut Out và kích thước ảnh PNG/JPEG. `npm run check` đạt **34 bài frontend**. `cargo test` đạt **65 bài**, 1 bài Win32 tách biệt bị bỏ qua. Clippy và định dạng đạt.
- Rust kiểm tra phiên chụp chặn lần mở lại trước khi đóng. Kiểm thử trực tiếp vị trí pill, hộp thoại lưu và thao tác chép vào clipboard trên Windows vẫn **Not Verified** cho bản sửa này.
