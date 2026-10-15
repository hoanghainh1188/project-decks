# Bài giới thiệu v1.3.0

Cùng một nội dung, bốn bản:

- `vi.md`: tiếng Việt.
- `en.md`: tiếng Anh.
- `qiita.md`: tiếng Nhật, front matter của Qiita CLI, khung ghi chú `:::note`.
- `zenn.md`: tiếng Nhật, front matter của Zenn (`emoji`, `type: "tech"`, `topics`, `published: false`), khung ghi chú `:::message`.

Bản tiếng Nhật có thêm một mục kỹ thuật, "平準化を 1,000 倍速くした方法", vì Qiita và Zenn là trang cho lập trình viên. Ảnh của bản tiếng Nhật là `images/sheafly-1.3.jpg` (chép từ `docs/images/screenshot-en.jpg`); `vi.md` và `en.md` dùng thẳng ảnh trong `docs/images/`. Phần "Hướng phát triển" là dự định, chưa có ngày; sửa lại nếu lộ trình đổi trước khi đăng.

## Qiita

**Đã đăng (15/10/2026):** https://qiita.com/harryforge/items/517c0568c11115a0d3a2. `qiita.md` mang `id` của bài này, nên `npx qiita publish` cập nhật bài đó thay vì tạo bài mới; bản dùng để đăng là `~/qiita-articles/public/sheafly-v1-3.md`, với ảnh trỏ tới `raw.githubusercontent.com` ở commit `2472c44`. Sửa bài thì sửa cả hai nơi.

Đăng lần đầu bằng trình soạn trên web:

1. Mở https://qiita.com/drafts/new.
2. Dán tiêu đề và 5 thẻ (`Rust`, `Tauri`, `React`, `プロジェクト管理`, `OSS`) từ front matter.
3. Dán phần thân, tức là mọi thứ sau dòng `---` thứ hai.
4. Kéo `images/sheafly-1.3.jpg` vào trình soạn, rồi thay dòng `![…](images/sheafly-1.3.jpg)` bằng đường dẫn Qiita sinh ra. Xóa dòng chú thích `<!-- … -->` phía trên.
5. Xem trước, rồi bấm đăng.

Hoặc Qiita CLI: chép `qiita.md` vào thư mục `public/` mà `npx @qiita/qiita-cli init` tạo ra (đăng nhập bằng `npx qiita login`), rồi chạy:

```bash
npx qiita publish qiita
```

## Zenn

Cách 1, trình soạn trên web:

1. Tạo bài mới trên https://zenn.dev.
2. Chọn emoji 📅, loại "Tech" và các topic `rust`, `tauri`, `react`, `projectmanagement`, `oss`.
3. Dán phần thân.
4. Tải ảnh lên tại https://zenn.dev/dashboard/uploader, rồi thay đường dẫn `/images/sheafly-1.3.jpg`.

Cách 2, kho GitHub đã liên kết với Zenn:

1. Chép `zenn.md` thành `articles/sheafly-v1-3-capacity.md`. Tên tệp chính là slug: 12–50 ký tự, gồm `a-z`, `0-9`, `-` và `_`.
2. Chép ảnh vào `images/sheafly-1.3.jpg` ở gốc kho.
3. Đổi `published: false` thành `true`, rồi push.
