# Bài giới thiệu v1.0.0 (tiếng Nhật)

Cùng một nội dung, hai bản theo cú pháp của từng trang:

- `qiita.md`: front matter của Qiita CLI, khung ghi chú `:::note`.
- `zenn.md`: front matter của Zenn (`emoji`, `type: "tech"`, `topics`, `published: false`), khung ghi chú `:::message`.
- `images/sheafly-gantt.jpg` (`projectdesk-gantt.jpg` là cùng ảnh, giữ lại vì bài Qiita đã đăng đang trỏ tới nó; xóa được sau khi đăng lại bài): ảnh chụp màn hình dùng trong bài.

Chỉ đăng sau khi release v1.0.0 đã publish, vì bài dẫn tới trang Releases.

## Qiita

Cách nhanh nhất là dùng trình soạn trên web:

1. Mở https://qiita.com/drafts/new.
2. Dán tiêu đề và 5 thẻ (`Rust`, `Tauri`, `React`, `TypeScript`, `OSS`) từ front matter.
3. Dán phần thân, tức là mọi thứ sau dòng `---` thứ hai.
4. Kéo `images/sheafly-gantt.jpg` vào trình soạn, rồi thay dòng `![…](images/sheafly-gantt.jpg)` bằng đường dẫn Qiita sinh ra. Xóa dòng chú thích `<!-- … -->` phía trên.
5. Xem trước, rồi bấm đăng.

Hoặc dùng Qiita CLI (bạn tự đăng nhập bằng token của mình):

```bash
npx @qiita/qiita-cli init
```

```bash
npx qiita login
```

Chép `qiita.md` vào thư mục `public/` mà CLI tạo ra, rồi chạy:

```bash
npx qiita publish qiita
```

## Zenn

Cách 1, trình soạn trên web:

1. Tạo bài mới trên https://zenn.dev.
2. Chọn emoji 📅, loại "Tech" và các topic `rust`, `tauri`, `react`, `typescript`, `oss`.
3. Dán phần thân.
4. Tải ảnh lên tại https://zenn.dev/dashboard/uploader, rồi thay đường dẫn `/images/sheafly-gantt.jpg`.

Cách 2, kho GitHub đã liên kết với Zenn:

1. Chép `zenn.md` thành `articles/sheafly-v1-rust-tauri.md`. Tên tệp chính là slug: 12–50 ký tự, gồm `a-z`, `0-9`, `-` và `_`.
2. Chép ảnh vào `images/sheafly-gantt.jpg` ở gốc kho.
3. Đổi `published: false` thành `true`, rồi push.
