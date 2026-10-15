---
title: "Sheafly 1.3.0: lập kế hoạch theo năng lực thật của đội"
tags: [Sheafly, quản lý dự án, Gantt, mã nguồn mở, Rust, Tauri]
---

# Sheafly 1.3.0: lập kế hoạch theo năng lực thật của đội

**Sheafly** là phần mềm lập kế hoạch dự án chạy trên máy tính (macOS, Windows, Linux), làm việc theo cách của Microsoft Project: bạn khai báo công việc, phụ thuộc, lịch làm việc và nguồn lực, còn Sheafly tự tính ngày, đường găng, chi phí, tình trạng quá tải và các chỉ số EVM. Phần mềm miễn phí, mã nguồn mở (giấy phép MIT), giao diện tiếng Việt và tiếng Anh.

Bản **1.3.0** vừa phát hành xoay quanh một câu hỏi mà hầu hết người lập kế hoạch đều gặp: *kế hoạch này có khớp với những người sẽ làm nó không?*

![Sheafly với bảng công việc, biểu đồ Gantt và bảng chỉ số](../../images/screenshot-vi.jpg)

Tải bản cài: [github.com/hoanghainh1188/sheafly/releases](https://github.com/hoanghainh1188/sheafly/releases)

---

## Sheafly làm được gì

### Lập lịch như Microsoft Project

- Phụ thuộc FS, SS, FF, SF kèm độ trễ; tính đường găng (CPM) và độ trễ cho phép (slack).
- Ràng buộc ngày (SNET, MSO, MFO…), hạn chót, công việc lên lịch tay, công việc tách đoạn, công việc lặp lại.
- Ba kiểu công việc Fixed Duration, Fixed Units, Fixed Work và cờ effort-driven: đổi một đại lượng, Sheafly tính lại hai đại lượng còn lại.
- Lịch làm việc theo giờ, ngày nghỉ, ngoại lệ, tuần làm việc riêng cho một khoảng ngày. Lịch có thể dựa trên lịch khác, nên một ngày lễ của dự án tự áp dụng cho mọi lịch nguồn lực.

### Theo dõi tiến độ và chi phí

- Đường cơ sở 0–10, so sánh kế hoạch với thực tế trên Gantt theo dõi.
- Ngày bắt đầu và kết thúc thực tế dời lịch đúng như Project; lệnh "Dời phần chưa xong sau ngày trạng thái".
- Chi phí theo giờ công và chi phí cố định; chi phí thực tế có thể nhập tay cho công việc hoặc từng phân công.
- Báo cáo **EVM** (PV, EV, AC, SPI, CPI, EAC…), **burn-down** và **đường cong S**.

### Nhiều cách nhìn

Gantt, Gantt theo dõi, bảng nguồn lực, mức sử dụng công việc, sơ đồ mạng, và mới ở bản này: **bảng tải nguồn lực**. Lọc, nhóm, tô sáng theo bất kỳ trường nào; xuất Gantt ra PDF có tiếng Việt đầy đủ.

### Dữ liệu an toàn

- Tệp `.sheafly` lưu cả lịch sử hoàn tác: mở lại hôm sau vẫn Undo được.
- Ghi tệp nguyên tử: mất điện giữa lúc lưu không làm hỏng tệp.
- Tự lưu mỗi 30 giây và khôi phục sau khi ứng dụng bị tắt đột ngột.
- Tệp bị hỏng một phần vẫn mở được tới trạng thái cuối cùng dựng lại được, kèm báo cáo.

### Trao đổi với công cụ khác

- Nhập và xuất **MSPDI (XML)** để làm việc cùng Microsoft Project: hạn chót, tiến độ, công thực tế, chi phí, tuần làm việc, khả dụng của nguồn lực, trường tùy chỉnh.
- Sau khi nhập, Sheafly liệt kê những gì đọc khác hoặc bỏ qua, ví dụ công việc bị dời so với ngày trong tệp.
- **Mới:** nhập công việc hoặc nguồn lực từ **CSV, Excel**.

---

## Có gì mới trong 1.3.0

### Lập kế hoạch theo năng lực

Đây là phần lớn nhất của bản này. Ý tưởng: thay vì xếp công việc rồi mới phát hiện ai đó bị giao 16 giờ một ngày, hãy cho Sheafly biết ai làm được gì và rảnh lúc nào, rồi để nó đề xuất.

- **Khả dụng theo ngày.** Mỗi người có các khoảng thời gian với mức năng lực riêng: nghỉ phép, làm bán thời gian, vào dự án muộn hơn. Nhập được theo đơn vị hoặc theo giờ mỗi ngày ("4h/ngày").
- **Kỹ năng và quy tắc phân công.** Đặt tên kỹ năng, gán cho từng người, rồi viết quy tắc chọn người (hoặc mọi người có kỹ năng đó) dựa trên trường tùy chỉnh của công việc.
- **Phân công theo quy tắc.** Sheafly gợi ý người cho các công việc chưa có ai, chia đều cho những người có kỹ năng. **Trước khi áp dụng**, bạn thấy công việc nào bị dời, hạn chót nào bị lỡ, ai còn quá tải. Áp dụng xong vẫn hoàn tác được bằng một lần Undo.
- **Việc khẩn cấp.** Đánh dấu "Chen ngang" và khi cân bằng nguồn lực, công việc này được làm trước việc ít ưu tiên hơn đang dở; việc đang dở bị tách quanh nó và Gantt vẽ khoảng nghỉ bằng nét đứt.
- **Bảng tải nguồn lực.** Công của từng người so với năng lực, theo ngày hoặc tuần. Bấm vào một người để xem họ đang làm gì.
- **Vượt mốc.** Lọc hoặc tô sáng những việc kết thúc sau một ngày, một mốc, hay sau hạn chót của chính nó: tiện để biết việc gì không kịp sprint.

### Cân bằng nguồn lực trong vài chục mili giây

Cân bằng 5.000 công việc trên 500 người giờ mất khoảng **40 ms**, trước đây là **46 giây**. Thứ tự xếp (ngày bắt đầu, ưu tiên, slack, hạn chót, thời lượng, ID hay trường tùy chỉnh, tăng hoặc giảm) chỉnh được trong Thông tin dự án → Cân bằng.

### Trường tùy chỉnh

Trường văn bản, số và danh sách lựa chọn cho công việc: thành cột trong bảng, nhóm trong bảng chi tiết, dùng để lọc, nhóm, điền xuống, và xuất sang Microsoft Project dưới dạng extended attribute.

### Và nhiều thứ khác

- Giao diện tối (theo hệ thống, hoặc luôn sáng, luôn tối).
- Sao chép, dán cả cây công việc kèm phân công và liên kết giữa hai dự án; kéo từng đoạn của công việc tách trên Gantt.
- Hộp thoại Thông tin dự án có tab Thống kê; hộp thoại Lịch làm việc sửa mọi lịch.
- macOS: không còn thông báo "ứng dụng bị hỏng"; hệ thống cho chọn **Vẫn mở**.

Chi tiết đầy đủ ở [ghi chú phát hành 1.3.0](https://github.com/hoanghainh1188/sheafly/releases/tag/v1.3.0).

---

## Vì sao Sheafly

- **Chạy trên máy bạn.** Không cần tài khoản, không cần máy chủ, dữ liệu dự án ở trong tệp của bạn.
- **Một bộ máy tính toán duy nhất.** Mọi con số (ngày, slack, chi phí, EVM) do engine viết bằng Rust tính ra; giao diện chỉ hiển thị. Cùng dữ liệu luôn cho cùng kết quả.
- **Nhanh với dự án lớn.** Mỗi chỉnh sửa lập lịch lại toàn bộ dự án; hàng nghìn công việc vẫn phản hồi tức thì.
- **Nói tiếng Việt thật sự.** Ngày `05/01/2026`, số `3.480.000`, thời lượng `7,5 ngày`; tiếng Anh dùng đúng thuật ngữ của Microsoft Project.

---

## Hướng phát triển tiếp theo

Những hướng chúng tôi đang hướng tới. Đây là dự định, chưa có ngày cụ thể, và thứ tự có thể thay đổi theo góp ý của người dùng.

### Phân tích rủi ro tiến độ bằng Monte Carlo

Một ngày kết thúc duy nhất thường gây hiểu lầm. Bạn sẽ nhập ước lượng dạng khoảng (thấp–cao) cho từng công việc, Sheafly chạy hàng nghìn lần mô phỏng trên chính bộ xếp lịch và cân bằng nguồn lực, rồi cho biết **P50, P90** của ngày kết thúc: "50% khả năng xong trước ngày X, 90% trước ngày Y". Công việc có hạn chót có nguy cơ bị lỡ sẽ được gắn cờ. Kết quả là số tính ra, không làm thay đổi kế hoạch của bạn, và cùng dữ liệu luôn cho cùng con số.

Việc tăng tốc cân bằng nguồn lực trong bản 1.3.0 chính là bước chuẩn bị cho tính năng này: mỗi lần mô phỏng phải đủ nhanh để vài nghìn lần chạy xong trong chưa đầy một phút.

### Khuyến nghị từ AI

Kết nối với các mô hình AI để đọc kế hoạch và đưa ra khuyến nghị: công việc nào đang là nút thắt, nên chuyển việc cho ai, phụ thuộc nào có vẻ thiếu, rủi ro nào nên để ý. Nguyên tắc giữ nguyên như phần phân công theo quy tắc: AI **đề xuất**, bạn xem trước và quyết định, mọi thay đổi vẫn hoàn tác được. Con số vẫn do engine tính, không phải do AI đoán.

### Kết nối với JIRA, Backlog, Asana

Đồng bộ công việc giữa Sheafly và các hệ thống quản lý công việc mà đội đang dùng: kéo issue, người phụ trách, ước lượng và trạng thái về để lập kế hoạch tổng thể và theo dõi tiến độ; đẩy ngày dự kiến ngược lại. Đội vẫn làm việc hằng ngày trên công cụ quen thuộc, còn người quản lý có cái nhìn về lịch, năng lực và rủi ro trên Sheafly.

### Những việc khác

- Ký số và notarize bản cài, để cài đặt không còn phải qua bước "Vẫn mở".
- Hiệu năng hiển thị bảng và Gantt với hàng chục nghìn dòng.
- Thêm ngôn ngữ giao diện (ví dụ tiếng Nhật).

---

## Thử ngay

| Hệ điều hành | Tệp |
|---|---|
| macOS (Apple Silicon và Intel) | `Sheafly_1.3.0_universal.dmg` |
| Windows | `.msi` hoặc `-setup.exe` |
| Linux | `.AppImage`, `.deb` hoặc `.rpm` |

Bản cài chưa được ký bằng chứng chỉ nhà phát triển:

- **macOS:** mở Sheafly một lần, rồi vào **Cài đặt hệ thống → Quyền riêng tư & Bảo mật** và bấm **Vẫn mở**.
- **Windows:** nếu SmartScreen hỏi, chọn **Thông tin thêm → Vẫn chạy**.

Sheafly 1.3.0 mở được mọi dự án từ 1.0.0 tới 1.2.0. Dự án đã lưu bằng 1.3.0 thì không mở được ở bản cũ hơn.

Góp ý, báo lỗi hoặc đề xuất tính năng: [GitHub Issues](https://github.com/hoanghainh1188/sheafly/issues). Đặc biệt, nếu đội bạn dùng JIRA, Backlog hay Asana, chúng tôi rất muốn nghe bạn muốn đồng bộ những gì.
