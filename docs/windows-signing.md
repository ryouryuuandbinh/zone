# Ký và xác minh bản Zone cho Windows

Smart App Control có thể chặn bản Zone chưa ký khi Windows chưa xác minh được độ tin cậy. Windows không có ngoại lệ Smart App Control cho từng ứng dụng. Bản build hoặc installer cần chữ ký Authenticode từ nhà cung cấp được Windows tin cậy. [Microsoft về Smart App Control](https://support.microsoft.com/en-us/windows/security/threat-malware-protection/smart-app-control-frequently-asked-questions) · [Yêu cầu về chữ ký](https://learn.microsoft.com/en-us/windows/apps/develop/smart-app-control/code-signing-for-smart-app-control)

Tại thời điểm kiểm tra, `src-tauri/target/release/zone.exe` có trạng thái `NotSigned` và mã SHA-256 khớp [bản build đã ghi](../artifacts/release-build.json). SignTool đã có trong Windows SDK trên máy. Kho chứng chỉ cá nhân của người dùng và máy đều chưa có chứng chỉ ký mã.

## Ký tệp chạy bằng chứng chỉ đã được cấp

Khi chứng chỉ do nhà cung cấp tin cậy cấp đã có trong kho cá nhân của người dùng và khóa ký khả dụng, điền dấu vân tay của chứng chỉ và URL đóng dấu thời gian RFC 3161 của nhà cung cấp. Chạy từ thư mục gốc của Zone. Lệnh tạo bản sao để ký và giữ riêng bản build gốc.

```powershell
$signTool = 'C:\Program Files (x86)\Windows Kits\10\bin\10.0.26100.0\x64\signtool.exe'
$thumbprint = '<dấu vân tay của chứng chỉ ký mã>'
$timestamp = '<URL RFC 3161 của nhà cung cấp>'
$source = (Resolve-Path -LiteralPath 'src-tauri/target/release/zone.exe').Path
$signed = Join-Path (Get-Location) 'artifacts/zone-signed.exe'

if (Test-Path -LiteralPath $signed) { throw 'Đã có tệp zone-signed.exe.' }
Copy-Item -LiteralPath $source -Destination $signed -ErrorAction Stop
& $signTool sign /sha1 $thumbprint /fd SHA256 /tr $timestamp /td SHA256 $signed
if ($LASTEXITCODE -ne 0) { throw 'Ký Zone thất bại.' }

& $signTool verify /pa /v $signed
if ($LASTEXITCODE -ne 0) { throw 'Chữ ký của Zone chưa hợp lệ.' }
Get-AuthenticodeSignature -LiteralPath $signed | Select-Object Status, SignerCertificate
```

Với dịch vụ ký từ xa, dùng quy trình của nhà cung cấp để ký và dùng bước xác minh trên cho tệp nhận lại. Với installer NSIS, cấu hình ký Windows trong Tauri để ký cả tệp ứng dụng lẫn installer. Sau khi xác minh chữ ký, thử chạy đúng bản đã ký trên máy đang bật Smart App Control. [SignTool](https://learn.microsoft.com/en-us/windows/win32/seccrypto/signtool) · [Ký Windows bằng Tauri](https://v2.tauri.app/distribute/sign/windows/)
