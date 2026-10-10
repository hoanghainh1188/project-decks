# ProjectDesk

Ứng dụng lịch trình kiểu Microsoft Project cho macOS (Tauri 2, React, Rust).

## Chạy

Cần Rust (`rustup`) và pnpm.

```bash
cd ~/Projects/projectdesk
pnpm install
pnpm tauri dev
```

Tệp dự án dùng đuôi `.pdproj`. Xuất/nhập lịch qua MSPDI (`.xml`).
