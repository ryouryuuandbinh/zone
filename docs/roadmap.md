# Roadmap

## Phase 0 — Nền tảng

Repository Tauri 2 + React + TypeScript strict + Vite, Motion, Tailwind, Zustand, Lucide, lint/format/typecheck, kiến trúc và hướng dẫn chạy. Dependency lockfiles là nguồn xác định phiên bản đã resolve; không tự nâng cấp chỉ vì registry có bản mới.

## Phase 1 — Island MVP

Overlay Windows trong suốt không viền, compact/expanded, spring morphing, hover/click, thông báo nội bộ, tray, Settings riêng và preferences cục bộ. Native hit testing dùng canvas cố định và cursor observer, không cắt cạnh WebView2 bằng region pixel. Các kiểm thử native còn thiếu là điều kiện chặn việc tuyên bố Phase 1 hoàn tất; xem [testing.md](testing.md).

## Phase 2 — Productivity

Đã triển khai clipboard text/image/file bằng Win32 clipboard events, tìm kiếm/ghim/sao chép lại/xóa, notes/todo/bản nháp, bookmark/category/native picker/drag-drop/open và bốn phím tắt cấu hình được. Monitoring tắt mặc định, lịch sử phiên chưa ghim ở RAM, full persistence tùy chọn; cache và payload có giới hạn. Lọc định dạng nhạy cảm khi nguồn cung cấp tín hiệu; không upload. Bộ test dữ liệu và build phải qua trước runtime QA. Chỉ đánh dấu nghiệm thu các đường tương tác thực sự đã thử trong [testing.md](testing.md).

## Phase 3 — Capture & media

Zone đóng băng màn hình đang hiển thị Island và mở luồng chọn vùng. Sau khi kéo, người dùng có thể đổi vùng, chú thích, sao chép, lưu PNG/JPEG hoặc ghim ảnh. Đánh giá Windows Graphics Capture và pipeline encode trước khi triển khai recorder. Media dùng Windows media session events.

## Phase 4 — Connectivity

LAN transfer, pairing bằng QR và phone-to-PC. Mỗi phiên cần xác thực, xác nhận nhận file, giới hạn quyền, xử lý tên file an toàn, không ghi đè tự động, hủy và lỗi có thể hiểu được. Không upload dịch vụ bên ngoài mặc định.

## Phase 5 — Advanced

Agent monitor từ sự kiện/integration hợp lệ của Codex và Claude Code, không suy luận chỉ từ process. Companion và plugin interface được thêm sau khi core ổn định. Không tự đọc lịch sử hoặc secrets của agent.

## Phase 6 — Release

Đo animation, CPU, bộ nhớ và startup trên bản release; chạy ma trận Windows 10/11, DPI/multi-monitor, soak test và crash recovery. Sau đó mới đóng gói installer, ký bản phát hành và công bố hướng dẫn nâng cấp.
