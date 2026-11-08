# Đo hiệu năng

Không suy ra FPS, CPU hoặc RAM từ việc build thành công. Mục tiêu 60 FPS và hỗ trợ màn hình 120 Hz chỉ là mục tiêu cho đến khi có trace và kết quả đo trên thiết bị cụ thể.

## Quy trình tái lập

1. Build bản release và ghi commit/source state, phiên bản Windows, CPU/GPU, RAM, độ phân giải, DPI, refresh rate, WebView2 và nguồn điện.
2. Chạy ít nhất ba lần cold start và ba lần warm start. Đo từ lúc tạo process đến khi Island hiển thị và nhận input; ghi riêng thời gian compile/dev server nếu đang dùng dev build.
3. Để idle hai phút, lấy CPU, private commit và working set trong ít nhất 60 giây. Tính tổng ứng dụng cùng các child process WebView2 của nó; không cộng nhầm WebView2 của ứng dụng khác. Tổng working set có thể đếm lại các trang chia sẻ; private commit không phải private working set.
4. Ghi trace 30 chu kỳ mở/đóng, 30 lần đảo chiều nhanh, notification đến khi panel đang mở và thay Settings giữa animation. Dùng Windows Performance Recorder/Analyzer hoặc công cụ frame profiling thích hợp.
5. Kiểm tra frame-time percentile, missed frames và CPU/GPU spikes. Theo dõi GDI object count để phát hiện region leak. Đo ở 60 Hz và 120 Hz thực nếu có; thay refresh rate không được coi là đã đo ở màn hình khác.
6. Chạy soak test ít nhất hai giờ, so sánh bộ nhớ/tài nguyên sau warm-up và trước khi đóng. Đóng từ tray, xác nhận process cùng các tác vụ nền kết thúc.

## Mẫu báo cáo

| Chỉ số                 | Bối cảnh                                        | Kết quả      |
| ---------------------- | ----------------------------------------------- | ------------ |
| Startup đến tương tác  | Release, cold/warm, ≥3 lần                      | Not Verified |
| CPU idle               | App + child WebView2, 60 giây                   | Not Verified |
| RAM idle               | Private commit và tổng working set, sau warm-up | Not Verified |
| Frame time p50/p95/p99 | 30 chu kỳ, 60 Hz                                | Not Verified |
| Frame time p50/p95/p99 | 30 chu kỳ, 120 Hz                               | Not Verified |
| GDI objects            | Trước/sau 1.000 lần chuyển trạng thái           | Not Verified |
| Tăng trưởng bộ nhớ     | Soak ≥2 giờ                                     | Not Verified |

Các ô trên chỉ được cập nhật khi lưu kèm phép đo, cách đo và cấu hình máy. Screenshot không chứng minh FPS hoặc không có flicker trong chuyển động.

## Mẫu debug đã thực hiện

Ngày 05/11/2026, bản embedded debug đã được chạy ở display scaling 200%. Startup diagnostic quan sát được là **513 ms**, tính từ Rust `run()` đến `island_ready`; không gồm Windows process launcher và chưa đo cold/warm start lặp lại.

[Artifact idle gốc](../artifacts/idle-performance.json) được ghi lúc 20:04 theo Asia/Saigon, lấy mẫu 10,31 giây:

| Chỉ số                     | Kết quả           | Giới hạn                                                     |
| -------------------------- | ----------------- | ------------------------------------------------------------ |
| Process được cộng          | 8                 | Zone và các process con WebView2; cả 8 có mặt ở hai snapshot |
| Logical CPU dùng chuẩn hóa | 18                | Số CPU khả dụng cho process lấy mẫu                          |
| CPU chuẩn hóa              | 0,0% sau làm tròn | Chỉ khoảng lấy mẫu này; không phải cam kết luôn bằng 0       |
| Private commit             | 188,07 MiB        | Không phải private working set                               |
| Tổng working set           | 428,8 MiB         | Có thể cộng lặp các trang dùng chung                         |

Mẫu ngắn trong debug build không đại diện cho release, không thay thế warm-up/soak test và không đo FPS. Mẫu này được lấy trước đợt chỉnh animation/timer; mẫu Phase 2 được ghi riêng bên dưới. Các mục benchmark release ở bảng trên vẫn **Not Verified**.

### Mẫu Phase 2 ngày 06/11/2026

[Artifact Phase 2](../artifacts/phase2-idle-performance.json) lấy lúc 21:22 Asia/Saigon, sau khi chạy các bảng và trở về pill; monitoring clipboard tắt, dữ liệu thử đã dọn. Khoảng mẫu **10,363 giây**, 7 process Zone/WebView2, cả 7 được ghép theo PID và creation time ở hai snapshot. CPU chuẩn hóa theo 18 logical CPU là **0,0% sau làm tròn**, private commit **276,50 MiB**, tổng working set **535,43 MiB**. Windows subsystem đã bỏ console của debug executable.

Đây là smoke measurement ngắn của bản debug, không chứng minh mức sử dụng RAM release hoặc tốc độ frame. Tổng working set có thể đếm lặp trang chia sẻ; private commit không phải private working set. Không so sánh trực tiếp với mẫu 05/11 như một regression benchmark vì số module, tiến trình và lịch sử tương tác khác nhau. Startup Phase 2, mẫu idle 60 giây, bộ nhớ khi có nhiều ảnh và FPS vẫn **Not Verified**.

## Mẫu release ngày 07/11/2026

[Artifact release](../artifacts/release-idle-performance.json) lấy lúc 08:07 Asia/Saigon, sau kiểm thử clipboard văn bản và trở về pill. Monitoring được người dùng bật, có một mục mẫu đã ghim, không mở Settings. Đã xác nhận executable ở `target/release/zone.exe`.

Khoảng mẫu **10,227 giây**, 7 process Zone/WebView2 có mặt ở cả hai snapshot. CPU chuẩn hóa theo 18 logical CPU khả dụng cho sampler là **0,0% sau làm tròn**, private commit **280,26 MiB**, tổng working set **534,05 MiB**.

Đây là smoke measurement ngắn trên máy hiện tại; không chứng minh CPU luôn bằng 0, không đo frame time và không thay thế benchmark 60 giây hoặc soak test. Tổng working set có thể cộng lặp trang chia sẻ; private commit không phải private working set. Không so sánh trực tiếp với các mẫu debug vì trạng thái và dữ liệu khác nhau. Các tiêu chí benchmark ở bảng đầu vẫn **Not Verified**.

## Công cụ lấy mẫu cục bộ

Sau khi có process Zone đang chạy, xác định đúng PID rồi dùng:

```powershell
.\scripts\Measure-Zone.ps1 -ZoneProcessId <PID> -DurationSeconds 10
```

Script đo CPU trong khoảng lấy mẫu, private commit và tổng working set của Zone cùng các process con WebView2. Kết quả ghi số process và những giới hạn của phép đo; CPU không bao gồm process được tạo rồi kết thúc giữa hai snapshot. Script cần quyền đọc thông tin process qua CIM. Ghi riêng build profile và đường dẫn executable đã xác nhận. Mẫu 10 giây chỉ là smoke measurement, không thay thế benchmark release 60 giây sau warm-up.

## Probe độ chính xác timer Windows

Để điều tra phản hồi mở/đóng chưa mượt, đã chạy [probe native](../artifacts/timer-probe.rs) với 60 lần chờ mỗi trường hợp trên máy hiện tại:

| Cấu hình                      | Yêu cầu chờ | Min      | Median   | Max      |
| ----------------------------- | ----------- | -------- | -------- | -------- |
| Timer mặc định                | 16 ms       | 16,07 ms | 31,08 ms | 31,93 ms |
| `timeBeginPeriod(1)` được cấp | 8 ms        | 8,03 ms  | 8,31 ms  | 9,24 ms  |

Probe đo thời gian `recv_timeout` thức dậy, không đo compositor, frame presentation hoặc FPS của Zone. Không quy đổi median 8,31 ms thành tuyên bố ứng dụng đạt 120 FPS.

Worker hiện chờ 8 ms trong transition và yêu cầu observer cập nhật native hit testing theo geometry mới nhất. Bản hiện tại không gọi SetWindowRgn hoặc native repaint mỗi frame. Yêu cầu timer precision 1 ms được giữ bằng RAII chỉ khi đang animation, được trả khi idle/thoát. [Microsoft](https://learn.microsoft.com/en-us/windows/win32/api/timeapi/nf-timeapi-timebeginperiod) yêu cầu ghép mỗi begin/end cùng period; Windows 11 không bảo đảm độ chính xác cao khi process bị che hoàn toàn hoặc ẩn. Tăng precision có thể ảnh hưởng CPU/điện năng, nên không giữ suốt vòng đời ứng dụng.

## Các ràng buộc thiết kế

- Không render animation liên tục khi idle.
- Không gọi Rust IPC mỗi animation frame.
- Không resize HWND/WebView2 mỗi animation frame.
- Không giữ file hay screenshot lớn trong React state.
- Worker, native observer và timer có lifecycle rõ ràng, dừng khi ứng dụng thoát.
- Native geometry worker chỉ chạy khi có transition và phải ngừng khi settle; cursor observer dùng sự kiện di chuyển chuột.
