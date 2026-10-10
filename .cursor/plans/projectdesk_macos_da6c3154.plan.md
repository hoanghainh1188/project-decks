---
name: ProjectDesk macOS
overview: "ProjectDesk là ứng dụng desktop Tauri (Rust + React) quản lý dự án theo hướng Microsoft Project. Pha 1–8 đã xong trên main; pha 9 (công việc lặp) và pha 10 (chỉ số EVM và burn-down) chưa làm."
todos:
  - id: scaffold
    content: Tauri 2 + React + crate engine
    status: completed
  - id: engine-core
    content: CPM, lịch, phụ thuộc, ràng buộc, đường găng
    status: completed
  - id: persist-ipc
    content: undo/redo, file .pdproj, lệnh Tauri
    status: completed
  - id: gantt-ui
    content: bảng task, Gantt, menu, WBS (PR #3 đã merge)
    status: completed
  - id: resources
    content: "Pha 2: nguồn lực, assignment, Fixed Duration/Units/Work, Resource Sheet, Task Usage, quá tải"
    status: completed
  - id: tracking
    content: "Pha 3: actuals, baseline 0–10, Tracking Gantt, chi phí"
    status: completed
  - id: leveling
    content: "Pha 4: heuristic theo priority rồi slack, leveling delay, không tách task"
    status: completed
  - id: mspdi
    content: "Pha 5: đọc/ghi MSPDI, không reader .mpp"
    status: completed
  - id: calendar-ui
    content: "Pha 6: sửa lịch trên giao diện (tuần làm việc, ngày nghỉ, lịch dự án và lịch nguồn lực). Ô ngày giờ dùng bộ chọn ngày giờ thay vì gõ chữ."
    status: completed
  - id: filter-group
    content: "Pha 7: lọc, nhóm, highlight (đường găng, theo người, trễ hạn)"
    status: completed
  - id: gantt-pdf
    content: "Pha 8: in hoặc xuất PDF trang Gantt"
    status: completed
  - id: recurring
    content: "Pha 9: công việc lặp"
    status: pending
  - id: evm-burndown
    content: "Pha 10: chỉ số EVM và burn-down chart"
    status: pending
isProject: false
---

# ProjectDesk — quản lý dự án kiểu Microsoft Project

Pha 1–8 đã xong trên `main`. Pha 1 gồm bảng task, Gantt, menu và cột WBS (PR #3). Pha 2–5 (nguồn lực, theo dõi và chi phí, leveling, MSPDI) đã merge qua PR #5. Pha 6 (lịch trên giao diện) qua PR #6. Pha 7 (lọc, nhóm, highlight) qua PR #7. Pha 8 (PDF Gantt) qua PR #8; hộp thoại file được sửa trong PR #9 đã merge. Pha 9 và pha 10 chưa làm.

Giao diện tiếng Việt. Các pha mở rộng cùng một mô hình lịch trong engine Rust.

## Kiến trúc

```mermaid
flowchart LR
  UI[React task sheet va Gantt]
  IPC[Tauri commands]
  Doc[ProjectDocument va undo]
  Eng[Scheduler engine]
  File[File SQLite pdproj]
  Xml[MSPDI XML]
  UI --> IPC --> Doc
  Doc --> Eng
  Eng --> Doc
  Doc --> File
  Doc --> Xml
```

- **Engine Rust** (`crates/engine`) là nguồn sự thật, không phụ thuộc Tauri. Mọi ngày bắt đầu, kết thúc, slack, đường găng, EVM và số cho burn-down do engine tính. Giao diện chỉ gửi lệnh và vẽ kết quả. Test CPM bằng `cargo test`.
- **Lệnh + undo**: mỗi thao tác (thêm task, đổi phụ thuộc, indent, gán nguồn lực, sửa lịch) là một command. Undo/redo đi trên log này.
- **File `.pdproj`**: một file SQLite. Mở/lưu bằng hộp thoại native của macOS. Snapshot burn-down (pha 10) nằm trong cùng file.
- **Hai chế độ task**, giống Project: *tự lên lịch* (engine dời ngày) và *lên lịch tay* (giữ ngày người dùng ghim).
- **Thời gian**: mốc là timestamp; thời lượng là phút làm việc. Lịch (tuần làm việc, ngoại lệ, ngày nghỉ) đổi phút làm việc thành ngày giờ thật.

Quyết định CPM, đã nằm trong engine:

1. Sắp topo theo phụ thuộc; báo vòng lặp.
2. Forward pass: early start/finish, cộng thời lượng trên lịch làm việc, áp ràng buộc và lag.
3. Backward pass: late start/finish, total slack, free slack.
4. Đường găng: total slack nhỏ hơn hoặc bằng 0.
5. Summary lấy min start và max finish của con.

Mô hình đã chốt: dự án có ngày bắt đầu, lịch chuẩn (thứ Hai–thứ Sáu, 08:00–12:00 và 13:00–17:00), ngày trạng thái. Task có tên, WBS, outline level, milestone (thời lượng 0), summary (rollup từ con), ghi chú, priority. Phụ thuộc FS / SS / FF / SF kèm lag theo phút làm việc. Ràng buộc đủ 8 kiểu: ASAP, ALAP, SNET, SNLT, FNET, FNLT, MSO, MFO. Lịch dự án và lịch task dùng cùng một kiểu lịch; pha 2 gắn lịch nguồn lực vào kiểu đó.

Trao đổi file ngoài dùng MSPDI (pha 5). Reader `.mpp` không nằm trong các pha này.

## Pha 1 — đã xong

- Scaffold Tauri 2 + React + TypeScript + Vite; workspace Cargo với `crates/engine`, `src-tauri` (lệnh mở/lưu) và `src` (React).
- Engine: CPM, lịch làm việc, phụ thuộc, ràng buộc, đường găng.
- Lưu trữ: command log undo/redo, file SQLite `.pdproj`, lệnh Tauri áp command.
- Giao diện: bảng task, Gantt (thanh task, summary, milestone, mũi tên phụ thuộc, đường găng, thang ngày/tuần/tháng), menu macOS, phím tắt, cột WBS. PR #3 đã merge.

## Pha 2 đến Pha 5 — đã xong

Pha 2–5 đã xong trên `main` qua PR #5.

### Pha 2 — nguồn lực

Đã xong.

- Resource: người hoặc máy, đơn vị tối đa, lịch riêng, mức giá chuẩn.
- Assignment: task gắn resource với units.
- Ba kiểu task của Project: Fixed Duration, Fixed Units, Fixed Work, kèm cờ effort-driven. Đổi một trong ba đại lượng duration / units / work thì engine tính lại hai đại lượng còn lại theo đúng kiểu task.
- View Resource Sheet và Task Usage (lưới theo ngày). Cột quá tải khi tổng units trong một khoảng vượt max units.

### Pha 3 — theo dõi, baseline, chi phí

Đã xong.

- Phần trăm hoàn thành, actual start/finish, remaining duration, remaining work.
- Baseline 0–10: mỗi mốc lưu start, finish, work, cost.
- Tracking Gantt: thanh kế hoạch và thanh baseline.
- Chi phí: standard rate, overtime rate, cost per use, fixed cost, cách cộng dồn (start / prorated / end).

Pha 10 đọc đúng các số này (baseline 0, ngày trạng thái, % complete, chi phí). UI không tính lại.

### Pha 4 — leveling

Đã xong. Phát hiện quá tải trước. Leveling là heuristic xếp theo priority rồi đến total slack (cùng họ với Project; không cam kết ra cùng một lịch từng giờ với Microsoft). Kết quả ghi vào field leveling delay. Pha này không tách task.

### Pha 5 — trao đổi với Microsoft Project

Đã xong.

- Đọc và ghi **MSPDI** (XML schema công khai của Project) bằng code của mình, cho tập field các pha 1–3 đã có: task, phụ thuộc, lịch, resource, assignment, baseline.
- Không có reader file nhị phân `.mpp` trong pha này. Parser Rust hiện có hoặc đọc một phần (MPP14), hoặc gắn giấy phép LGPL nên không link tĩnh vào app. Hướng đi đã chốt: người dùng Save As XML khi cần đủ field. Reader `.mpp` chỉ được xét lại khi có thư viện tương thích giấy phép và đọc đủ lịch cùng baseline — việc đó nằm ngoài mười pha.

## Pha 6 đến Pha 9

Pha 6–8 đã xong trên `main`. Pha 9 chưa làm.

### Pha 6 — sửa lịch trên giao diện

Đã xong qua PR #6.

- Các ô ngày giờ đang sửa được (ngày công việc, ngày thực, ngày ràng buộc, ngày bắt đầu dự án, ngày trạng thái, ngày nghỉ) dùng bộ chọn ngày giờ (`datetime-local` hoặc `date`) thay vì gõ chữ. Mốc thời gian trong engine vẫn là millisecond.
- Sửa tuần làm việc: ngày trong tuần và các khoảng giờ. Mặc định giữ thứ Hai–thứ Sáu, 08:00–12:00 và 13:00–17:00.
- Thêm, sửa, xóa ngày nghỉ và ngoại lệ (một ngày hoặc một khoảng Từ–Đến), nghỉ cả ngày hoặc có giờ làm riêng. Ngày của ngoại lệ chọn bằng bộ chọn ngày, không gõ tự do.
- Sửa lịch dự án, lịch task và lịch nguồn lực trên cùng kiểu lịch của engine. Chọn "lịch dự án" vẫn là kế thừa. Tạo hoặc sửa lịch có tên đi qua command nên undo/redo gồm các lần sửa đó.
- Mỗi lần sửa là một command (undo được). Engine tính lại sớm/muộn, slack và đường găng. Giao diện không tự đổi phút làm việc.

### Pha 7 — lọc, nhóm, highlight

Đã xong qua PR #7.

- Lọc bảng và Gantt theo ba tiêu chí: đường găng (total slack nhỏ hơn hoặc bằng 0), theo người được gán, task trễ hạn.
- Trễ hạn: finish hiện tại sau finish của baseline 0, hoặc task chưa xong mà finish đã qua ngày trạng thái.
- Nhóm theo nguồn lực hoặc theo summary (WBS). Nhóm chỉ đổi cách xếp hàng trên view, không ghi đè outline đã lưu.
- Highlight tô đúng ba nhóm trên. Cờ đường găng và trễ hạn lấy từ engine. Bộ lọc và nhóm là trạng thái cửa sổ.

### Pha 8 — in hoặc xuất PDF trang Gantt

Đã xong qua PR #8. Hộp thoại file được sửa trong PR #9 đã merge.

- In hoặc xuất PDF đúng trang đang xem: bảng task bên trái và Gantt bên phải.
- Giữ thang ngày/tuần/tháng, đường găng, mũi tên phụ thuộc, và bộ lọc pha 7 nếu đang bật.
- Cắt trang theo chiều thời gian và theo danh sách task.
- Hộp thoại in/lưu của macOS. Ngày trên trang in là ngày engine đã tính. Không có server.

### Pha 9 — công việc lặp — chưa làm

- Tạo chuỗi lặp theo ngày, tuần hoặc tháng, với số lần hoặc một ngày kết thúc chuỗi.
- Mỗi lần lặp là một task occurrence trên lịch làm việc, gắn task gốc. Summary của chuỗi rollup như summary thường. CPM áp dụng trên occurrence.
- Sửa cả chuỗi hoặc sửa một lần. Xóa chuỗi xóa các occurrence còn theo chuỗi.
- Sinh và sửa chuỗi đi qua command log. Không dùng tách task (split) để làm occurrence.

## Pha 10 — chỉ số EVM và burn-down — chưa làm

Pha 10 chưa làm. Cả panel EVM và burn-down chỉ đọc số do engine trả về. JavaScript không tính lại EVM.

### EVM

Nguồn số liệu là baseline 0, ngày trạng thái, % complete và chi phí đã định ở pha 3 (kể cả cách cộng dồn start / prorated / end). Cộng trên task lá để summary không bị tính hai lần.

- **BAC**: tổng cost baseline 0.
- **PV (BCWS)**: phần cost baseline 0 lẽ ra xong tính đến ngày trạng thái. Cộng dồn `start`: tính cả cost nếu baseline start đã tới. `end`: tính cả cost nếu baseline finish đã tới. `prorated`: tỷ lệ phút làm việc từ baseline start đến ngày trạng thái trên thời lượng baseline, nhân cost baseline.
- **EV (BCWP)**: tổng của % complete × cost baseline 0 của từng task lá.
- **AC (ACWP)**: tổng chi phí thực tế đến ngày trạng thái.
- **SV** = EV − PV. **CV** = EV − AC.
- **SPI** = EV / PV khi PV lớn hơn 0. **CPI** = EV / AC khi AC lớn hơn 0.
- **EAC** = BAC / CPI khi CPI lớn hơn 0. **ETC** = EAC − AC. **VAC** = BAC − EAC.
- Khi mẫu số không lớn hơn 0, engine trả SPI, CPI, EAC, ETC, VAC là không xác định. Không đổi sang công thức EAC khác.

Panel nhỏ cạnh Gantt, nhãn tiếng Việt và viết tắt trong ngoặc:

- Giá trị kế hoạch (PV, BCWS)
- Giá trị đạt được (EV, BCWP)
- Chi phí thực tế (AC, ACWP)
- Chênh lệch tiến độ (SV)
- Chênh lệch chi phí (CV)
- Chỉ số tiến độ (SPI)
- Chỉ số chi phí (CPI)
- Ngân sách khi hoàn thành (BAC)
- Ước tính khi hoàn thành (EAC)
- Ước tính phần còn lại (ETC)
- Chênh lệch khi hoàn thành (VAC)

### Burn-down

- Biểu đồ cùng khu vực cạnh Gantt: trục ngang là ngày, trục dọc là khối lượng còn lại (remaining work của pha 3). Engine lưu phút làm việc; UI có thể hiện giờ.
- Đường thực tế nối các snapshot. Đường lý tưởng là đoạn thẳng từ baseline start (khối lượng = tổng work baseline 0) đến baseline finish (khối lượng = 0).
- Mỗi lần người dùng lưu file, hoặc khi ngày trạng thái đổi, engine ghi thêm một snapshot: ngày, remaining work, EV, AC. Cùng một ngày thì ghi đè snapshot của ngày đó.
- Snapshot nằm trong SQLite `.pdproj`. Không có server, không có dịch vụ ngoài máy.

## Vẫn chưa làm

Tách task, đọc file `.mpp`, sơ đồ mạng, nhiều người dùng, server, công thức trường tùy chỉnh, CI bản cài Windows/Linux.
