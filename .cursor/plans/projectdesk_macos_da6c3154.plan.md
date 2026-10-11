---
name: ProjectDesk macOS
overview: "ProjectDesk là ứng dụng desktop Tauri (Rust + React) quản lý dự án theo hướng Microsoft Project. Pha 1–10 đã xong. Sau pha 10 đã thêm: bản cài đa nền tảng (PR #17), test frontend (PR #18), phần còn lại của pha 9–10 (PR #19), sơ đồ mạng (PR #20), tách công việc (PR #21), CLAUDE.md (PR #22) và an toàn dữ liệu (PR #23). Việc tiếp theo là Pha 11–17 ở mục Lộ trình."
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
    content: bảng task, Gantt, menu (PR #1), cột WBS (PR #3)
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
    status: completed
  - id: evm-burndown
    content: "Pha 10: chỉ số EVM và burn-down chart"
    status: completed
  - id: phase-9-10-followups
    content: "AC theo thời gian, đường cong S, thêm mẫu lặp, đơn vị nguồn lực của chuỗi (PR #19)"
    status: completed
  - id: split-task
    content: "Tách công việc (PR #21)"
    status: completed
  - id: network-diagram
    content: "Sơ đồ mạng kiểu PERT (PR #20)"
    status: completed
  - id: data-safety
    content: "Hỏi lưu khi còn thay đổi, lưu file nguyên tử, kiểm tra phiên bản tệp, phím tắt không cướp ô nhập (PR #23)"
    status: completed
  - id: ui-basics
    content: "Pha 11: giao diện cơ bản (đơn vị thời lượng, panel thuộc tính, nhãn ngày, cột và nhãn nguồn lực, giải thích ngày kết thúc)"
    status: pending
  - id: file-performance
    content: "Pha 12: nền tệp và hiệu năng (checkpoint, lệnh bất đồng bộ, khôi phục, autosave)"
    status: pending
  - id: actuals-drive-schedule
    content: "Pha 13: theo dõi tiến độ (ngày thực tế điều khiển lịch, trễ hạn theo MS Project)"
    status: pending
  - id: macos-release
    content: "Pha 14: phát hành macOS (ký số, notarize, universal, kiểu tệp .pdproj, CSP)"
    status: pending
  - id: mspdi-complete
    content: "Pha 15: MSPDI đầy đủ (Deadline, chi phí thực tế, WorkWeeks)"
    status: pending
  - id: leveling-hours
    content: "Pha 16: leveling theo giờ trùng nhau"
    status: pending
  - id: project-parity-ux
    content: "Pha 17: UX ngang MS Project (copy/paste, tách App.tsx, bàn phím, giao diện còn lại)"
    status: pending
isProject: false
---

# ProjectDesk — quản lý dự án kiểu Microsoft Project

Pha 1–10 đã xong. Lõi của pha 1–5 (engine CPM, lịch, nguồn lực, theo dõi và chi phí, leveling, MSPDI, bảng task, Gantt, menu) vào cùng lúc ở PR #1. PR #2 cấu hình CI và định danh app macOS. PR #3 thêm cột WBS. PR #4 thêm bản kế hoạch sửa đổi. PR #5 lấp các chỗ còn thiếu của pha 2, 3 và 5. Pha 6 (lịch trên giao diện) qua PR #6. Pha 7 (lọc, nhóm, highlight) qua PR #7. Pha 8 (PDF Gantt) qua PR #8; hộp thoại file được sửa trong PR #9 đã merge. Pha 9 (công việc lặp) qua PR #15 đã merge. Pha 10 (chỉ số EVM và burn-down) qua PR #16.

Các sửa bổ sung đã merge vào `main`: PR #10 (căn header với độ rộng ngày của Gantt), PR #11 và PR #13 (đồng bộ kế hoạch), PR #12 (bộ chọn ngày giờ trong app), PR #14 (nhóm theo summary ở pha 7).

Sau pha 10 (chi tiết ở mục Sau pha 10): PR #17 (CI build bản cài macOS, Windows, Linux), PR #18 (test frontend với Vitest), PR #19 (phần còn lại của pha 9–10), PR #20 (sơ đồ mạng), PR #21 (tách công việc), PR #22 (CLAUDE.md), PR #23 (an toàn dữ liệu).

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
- Giao diện: bảng task, Gantt (thanh task, summary, milestone, mũi tên phụ thuộc, đường găng, thang ngày/tuần/tháng), menu macOS, phím tắt (PR #1), cột WBS (PR #3).

## Pha 2 đến Pha 5 — đã xong

Pha 2–5 vào cùng lõi ở PR #1; PR #5 lấp các chỗ còn thiếu (effort-driven, chi phí summary, giờ ngoại lệ trong MSPDI).

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

Pha 6–9 đã xong trên `main`.

### Pha 6 — sửa lịch trên giao diện

Đã xong qua PR #6. Bộ chọn ngày giờ được thay bằng popover trong app ở PR #12.

- Các ô ngày giờ đang sửa được (ngày công việc, ngày thực, ngày ràng buộc, ngày bắt đầu dự án, ngày trạng thái, ngày nghỉ) dùng bộ chọn ngày giờ dạng popover viết trong app (lưới tháng bắt đầu từ thứ Hai, nút hôm nay, chọn giờ và phút, nút xóa cho ngày không bắt buộc). Lý do: cửa sổ Tauri trên macOS (WKWebView) không mở bộ chọn gốc cho ô nhập ngày/giờ. Vẫn gõ ngày trực tiếp được. Mốc thời gian trong engine vẫn là millisecond.
- Sửa tuần làm việc: ngày trong tuần và các khoảng giờ. Mặc định giữ thứ Hai–thứ Sáu, 08:00–12:00 và 13:00–17:00.
- Thêm, sửa, xóa ngày nghỉ và ngoại lệ (một ngày hoặc một khoảng Từ–Đến), nghỉ cả ngày hoặc có giờ làm riêng. Ngày của ngoại lệ chọn bằng bộ chọn ngày, không gõ tự do.
- Sửa lịch dự án, lịch task và lịch nguồn lực trên cùng kiểu lịch của engine. Chọn "lịch dự án" vẫn là kế thừa. Tạo hoặc sửa lịch có tên đi qua command nên undo/redo gồm các lần sửa đó.
- Mỗi lần sửa là một command (undo được). Engine tính lại sớm/muộn, slack và đường găng. Giao diện không tự đổi phút làm việc.

### Pha 7 — lọc, nhóm, highlight

Đã xong qua PR #7. Nhóm theo summary được thêm ở PR #14.

- Lọc bảng và Gantt theo ba tiêu chí: đường găng (total slack nhỏ hơn hoặc bằng 0), theo người được gán, task trễ hạn.
- Trễ hạn: task bị coi là trễ nếu thỏa một trong các điều kiện: chưa xong và finish sau ngày trạng thái; finish sau finish của baseline 0; hoặc có đặt deadline và finish sau deadline. Deadline là một ngày tùy chọn của task, sửa bằng bộ chọn ngày, undo được, và không phải một kiểu ràng buộc.
  - Ghi chú: điều kiện đầu khác MS Project. Khi đã đặt ngày trạng thái, mọi task chưa xong có finish trong tương lai đều bị tính là trễ (`Task::is_late` trong `model.rs`). MS Project chỉ tính trễ khi tiến độ thực tế ít hơn phần lẽ ra đã xong tại ngày trạng thái. Sửa ở Pha 13.
- Nhóm có hai kiểu: theo người và theo summary (WBS). Theo người: task có nhiều người hiện dưới từng người, task chưa gán ai nằm trong nhóm "Chưa gán". Theo summary: task lá nằm dưới summary cha gần nhất, tiêu đề nhóm là mã WBS và tên (ví dụ "1.2 Thiết kế"), task lá cấp cao nhất nằm trong nhóm "Không có summary"; nhóm và task trong nhóm giữ thứ tự outline. Nhóm chỉ đổi cách xếp hàng trên view, không ghi đè outline đã lưu.
- Highlight tô đúng ba nhóm trên. Cờ đường găng và trễ hạn lấy từ engine. Bộ lọc và nhóm là trạng thái cửa sổ.

### Pha 8 — in hoặc xuất PDF trang Gantt

Đã xong qua PR #8. Hộp thoại file được sửa trong PR #9 đã merge.

- In hoặc xuất PDF đúng trang đang xem: bảng task bên trái và Gantt bên phải.
- Giữ thang ngày/tuần/tháng, đường găng, mũi tên phụ thuộc, và bộ lọc pha 7 nếu đang bật.
- Cắt trang theo chiều thời gian và theo danh sách task.
- Hộp thoại in/lưu của macOS. Ngày trên trang in là ngày engine đã tính. Không có server.

### Pha 9 — công việc lặp — đã xong (PR #15)

Đã xong qua PR #15 đã merge. Đã làm (module `recurrence.rs` trong engine, hộp thoại `RecurringDialog`):

- Mẫu lặp theo kiểu MS Project: ngày (mỗi N ngày làm việc trên lịch của chuỗi), tuần (mỗi N tuần vào các thứ đã chọn, tuần bắt đầu thứ Hai), tháng (ngày X của mỗi N tháng; X lớn hơn số ngày của tháng thì rơi vào ngày cuối tháng). Kết thúc sau N lần hoặc tại một ngày (tính cả ngày đó). Tối đa 999 lần như Project.
- Chuỗi là một summary task giữ định nghĩa (`Task.recurrence`). Con của nó là các occurrence thật (`series_uid`, `occurrence_date`), mỗi cái có ràng buộc SNET tại giờ làm việc đầu tiên của ngày mẫu sinh ra, theo lịch của chuỗi hoặc lịch dự án. Ngày nghỉ nên occurrence dời sang giờ làm việc kế tiếp. Summary rollup như summary thường, CPM chạy trên occurrence. Occurrence là fixed units, không effort-driven, nên gán nhiều nguồn lực không rút ngắn thời lượng.
- Lệnh `CreateRecurringTask` và `UpdateRecurringTask` đi qua command log, nên undo/redo và lưu `.pdproj` hoạt động. Đổi tên summary cũng sửa tên chuỗi và sinh lại.
- Sửa riêng một occurrence (tên, thời lượng, ngày, ràng buộc, lịch, nguồn lực, tiến độ, chi phí…) đánh dấu `detached`; sinh lại chuỗi giữ nguyên lần đó và không sinh trùng ngày của nó. Occurrence còn theo chuỗi giữ UID theo thứ tự khi sinh lại, nên liên kết và baseline trên chúng còn.
- Xóa một occurrence ghi ngày đó vào `skipped` để sinh lại không tạo lại. Xóa chuỗi xóa summary và các occurrence còn theo chuỗi; occurrence đã tách và task tự thêm bên dưới được giữ lại, lên một cấp.
- Không đổi cấp (thụt vào/ra) từng occurrence; phải sửa cả chuỗi.
- MSPDI: xuất occurrence như task thường dưới summary, summary có `<Recurring>1</Recurring>`. Nhập lại thành task thường (không dựng lại chuỗi).
- Giao diện: nút "Công việc lặp" trên thanh công cụ và mục "Sửa → Công việc lặp…". Hộp thoại có tên, thời lượng, mẫu, ngày bắt đầu và kết thúc (dùng DateTimeField), lịch, nguồn lực. Dòng summary có biểu tượng ↻, bấm để sửa chuỗi; occurrence có dấu ·, lần đã tách có ↺. Inspector có nút "Sửa cả chuỗi…".

Giới hạn:

- ~~Chưa có mẫu "mỗi N ngày lịch", "thứ X thứ Y trong tháng" (ví dụ thứ Hai đầu tiên) hay lặp theo năm.~~ Đã thêm ở PR #19 (`calendar_daily`, `monthly_nth`, `yearly`, `yearly_nth`).
- Nhập MSPDI không dựng lại chuỗi lặp; chuỗi thành summary thường với các task con.
- ~~Nguồn lực của chuỗi luôn gán với đơn vị 1.0.~~ PR #19 cho chọn đơn vị từng nguồn lực của chuỗi.
- ~~Chưa kiểm thử tự động phần giao diện.~~ PR #18 thêm test Vitest cho hộp thoại.

## Pha 10 — chỉ số EVM và burn-down — đã xong (PR #16)

Đã xong qua PR #16. Đã làm (module `evm.rs` trong engine, lệnh Tauri `project_metrics`, panel `EvmPanel`). Cả panel EVM và burn-down chỉ đọc số do engine trả về. JavaScript không tính lại EVM.

- Engine tính BAC, PV, EV, AC, SV, CV, SPI, CPI, EAC, ETC, VAC đúng công thức dưới đây, cộng trên task lá. Cách cộng dồn của PV lấy từ trường cộng dồn của task (Đầu / Phân bổ / Cuối), cùng trường pha 3 dùng cho chi phí thực tế. Với `prorated`, thời lượng baseline là số phút làm việc giữa baseline start và baseline finish trên lịch của task. Baseline thời lượng 0 (mốc) coi như đã lên kế hoạch hết khi tới baseline finish.
- Mẫu số không lớn hơn 0 thì SPI, CPI, EAC, ETC, VAC là `null`; giao diện hiện "—" kèm chú thích. Thiếu ngày trạng thái hoặc không task lá nào có baseline 0 thì engine không trả chỉ số; panel báo rõ thiếu gì và có nút mở hộp thoại tương ứng.
- Snapshot burn-down (ngày, remaining work, EV, AC) nằm trong bảng `burndown` của file `.pdproj`, ngoài command log nên undo không xóa. Ghi khi lưu tệp (Lưu, Lưu thành…) và khi một lệnh làm đổi ngày trạng thái sang một ngày khác rỗng. Ngày của snapshot là 00:00 của ngày trạng thái; chưa có ngày trạng thái thì lấy ngày hiện tại. Cùng ngày thì ghi đè. File cũ không có bảng thì mở với danh sách rỗng.
- Engine trả đường lý tưởng (baseline start, tổng work baseline 0 của task lá) → (baseline finish, 0), các snapshot có ngày không sau ngày trạng thái, và remaining work hiện tại.
- Giao diện: nút "EVM" trên thanh công cụ và mục "Xem → Chỉ số EVM và burn-down" mở panel bên phải Gantt (trong view Gantt và Gantt theo dõi). Panel có bảng 11 chỉ số với nhãn bên dưới, và biểu đồ burn-down vẽ tay bằng SVG cùng bảng màu với Gantt: đường lý tưởng nét đứt xám, đường thực tế xanh có điểm, ngày trạng thái nét đứt đỏ, trục dọc tính bằng giờ.
- Test engine trong `crates/engine/tests/engine.rs`: từng kiểu cộng dồn (kể cả giờ nghỉ trưa trên lịch), bộ chỉ số tính tay, chỉ cộng task lá, các trường hợp chia cho 0, đường lý tưởng, ghi và ghi đè snapshot, lưu/mở, undo, lọc theo ngày trạng thái, file cũ không có bảng.

Giới hạn:

- ~~AC không cắt theo thời gian tại ngày trạng thái.~~ PR #19 rải AC đều theo thời gian làm việc từ ngày bắt đầu thực tế tới điểm tiến độ, theo cách cộng dồn của task.
- Task lá chưa có baseline 0 (thêm sau khi đặt baseline) không góp vào BAC, PV, EV nhưng vẫn góp vào AC. Chi phí cố định đặt trên chính summary không được tính, vì chỉ cộng task lá.
- Đường thực tế chỉ có điểm từ lúc dùng bản này: không dựng lại lịch sử cho file cũ. Hoàn tác việc đổi ngày trạng thái không xóa snapshot đã ghi. Ngày "hiện tại" khi chưa có ngày trạng thái lấy theo giờ UTC, giống ngày bắt đầu mặc định của dự án mới.
- ~~Chưa vẽ đường cong S.~~ PR #19 thêm đường cong S (PV/EV/AC cộng dồn theo ngày hoặc tuần).
- ~~Chưa kiểm thử tự động phần giao diện.~~ PR #18 thêm test cho `EvmPanel`.

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

## Sau pha 10 — đã xong

- **Bản cài (PR #17):** `.github/workflows/build.yml` build bản cài chưa ký cho macOS, Windows, Linux; tag `v*` tạo draft release.
- **Test frontend (PR #18):** Vitest, Testing Library, jsdom. `src/test/fakeBackend.ts` thay các lệnh Tauri; không chạy engine thật.
- **Sơ đồ mạng (PR #20):** module `network.rs` xếp ô theo cột và tuyến cạnh; lệnh Tauri `network_layout`; view "Sơ đồ mạng" có kéo, phóng to, chọn chung với bảng task, hộp thoại sửa và xuất PDF. Mặc định ẩn summary.
- **Tách công việc (PR #21):** module `split.rs`. Một lần tách lưu số phút làm việc trước khoảng nghỉ và độ dài khoảng nghỉ; `schedule()` tính ra các đoạn (`segments`). Baseline lưu đoạn của nó, PV và burn-down kế hoạch bỏ qua khoảng nghỉ. MSPDI đọc/ghi các đoạn. Gantt vẽ thanh đứt đoạn.
- **CLAUDE.md (PR #22):** hướng dẫn cho Claude Code: lệnh build/test, kiến trúc, quy tắc của repo.
- **An toàn dữ liệu (PR #23):**
  - Mới, Mở, Nhập MSPDI, đóng cửa sổ, Cmd+Q và thoát từ Dock đều hỏi "Lưu / Không lưu / Hủy" khi còn thay đổi chưa lưu. Cmd+Q dùng mục menu riêng thay cho mục Thoát có sẵn.
  - Lưu ghi ra một tệp tạm ẩn, tên riêng, cạnh tệp thật (đi theo symlink) trong một transaction, giữ quyền của tệp cũ, rồi đổi tên đè lên. App crash giữa chừng không làm hỏng tệp, và không xóa nhầm tệp `.tmp` của người dùng.
  - Trong lúc một hộp thoại native đang mở, các lệnh sửa, hoàn tác, làm lại bị từ chối, để câu trả lời "Lưu / Không lưu" áp đúng trạng thái người dùng đang thấy.
  - Mở tệp kiểm tra `FORMAT_VERSION`: tệp của phiên bản mới hơn bị từ chối với thông báo rõ, và lệnh lạ trong log cũng báo như vậy. Thêm biến thể `Command` hoặc bảng mới thì phải tăng `FORMAT_VERSION`.
  - Đang gõ dở trong ô nhập thì Cmd+Z, Cmd+Shift+Z hoàn tác/làm lại chữ trong ô; ô không còn chữ chưa xác nhận thì chúng hoàn tác dự án như thường. Cmd+Backspace trong ô xóa chữ tới đầu dòng, không xóa công việc (`src/shortcuts.ts`).

## Lộ trình

Theo đánh giá độc lập sau PR #23 (đối chiếu từng mục với code) và kiểm tra giao diện trên một dự án mẫu 21 dòng. Pha 11 làm trước vì nhỏ và người dùng thấy ngay; Pha 12 là nền cho Pha 13 và 16. Pha 14 chạy song song khi có tài khoản Apple Developer.

### Pha 11 — giao diện cơ bản (S–M)

Mục tiêu: màn hình chính đọc đúng và đủ thông tin mà không phải đoán.

- Thời lượng và việc còn lại hiện theo ngày như Project ("7,5 ngày", mốc "0 ngày"), không lẫn "60 giờ", "768 phút" trong cùng bảng.
- Panel thuộc tính: có tiêu đề là tên task đang chọn, chia nhóm (Lịch, Theo dõi, Chi phí, Nguồn lực), thu gọn được, không hiện ở view Nguồn lực, Sử dụng, Sơ đồ mạng; task summary không hiện trường không áp dụng.
- Nhãn ngày ở thang "Ngày" không đè lên nhau.
- Bảng Gantt có cột Nguồn lực, % hoàn thành, Tiền nhiệm; thanh Gantt ghi tên nguồn lực bên phải như Project.
- Thanh trạng thái nói task nào quyết định ngày kết thúc dự án (ví dụ chuỗi họp lặp kéo dài sau mốc phát hành), để đường găng không gây hiểu lầm.

Nghiệm thu: test Vitest cho định dạng thời lượng, nhãn ngày và cột mới; chụp màn hình dự án mẫu trước và sau.

### Pha 12 — nền tệp và hiệu năng (L)

Mục tiêu: undo, mở tệp và sửa không làm đứng UI ở dự án lớn, và tệp hỏng một phần vẫn mở được.

- Hiện trạng đo được: dự án 600 task, 1.800 lệnh thì mỗi lần Undo mất khoảng 3,7 giây, mở tệp tương tự; thời gian tăng gần bậc hai (100 task 71 ms, 300 task 0,72 giây). `apply_command`, `undo`, `redo` là lệnh Tauri đồng bộ nên chạy trên luồng chính.
- Checkpoint `Project` trong bộ nhớ (khoảng 32 mốc) và trong tệp (bảng `checkpoint`, mỗi 50 lệnh và tại cursor khi lưu). `base` và log vẫn là nguồn sự thật; checkpoint chỉ là bộ đệm, bỏ qua khi khác phiên bản engine. Thêm bảng nên tăng `FORMAT_VERSION`.
- Lệnh Tauri sửa và hoàn tác chạy bất đồng bộ, ngoài luồng chính.
- Chỉ mục UID → vị trí; `collect_bounds` không quét toàn bộ phụ thuộc cho mỗi task lá.
- Một lệnh lỗi khi replay không làm mất cả tệp: mở ở checkpoint gần nhất, chỉ đọc, kèm cảnh báo.
- Autosave và khôi phục sau crash vào tệp ẩn cạnh tệp thật.
- Tùy chọn gộp lịch sử cũ để log không phình mãi.

Nghiệm thu: bench trong `crates/engine/benches` với 1.000 task, 10.000 lệnh: Undo dưới 100 ms, mở dưới 1 giây; test tệp có lệnh hỏng; test tắt app giữa chừng rồi khôi phục.

### Pha 13 — theo dõi tiến độ (L, cần Pha 12)

Mục tiêu: nhập tiến độ thực tế làm lịch thay đổi như MS Project.

- Ngày bắt đầu thực tế ghim start, bỏ qua phụ thuộc và ràng buộc; ngày kết thúc thực tế ghim finish. Phần còn lại nối liền sau phần đã làm.
- Lệnh "Dời phần chưa xong sau ngày trạng thái" dùng `split.rs` để tạo khoảng nghỉ, nên tất định và hoàn tác được.
- Task đã xong không nằm trên đường găng; leveling không dời task đã bắt đầu.
- Sửa cờ trễ hạn theo cách của MS Project (xem ghi chú ở Pha 7).
- `SetRemainingDuration` không xóa tiến độ: duration = phần đã làm + phần còn lại.
- AC và EV của task tách dùng các đoạn trong `progress_window`.
- Lệnh theo dõi trên occurrence của chuỗi lặp không đánh dấu `detached`; sinh lại không động vào occurrence đã có tiến độ.
- Nghĩa của lệnh cũ đổi nên tăng `FORMAT_VERSION`.

Chỗ phải sửa: `schedule_leaf` và `late_leaf` (`schedule.rs`), `work.rs` (bỏ ép `actual_finish = finish`), `SetPercentComplete` và `SetRemainingDuration` (`command.rs`), `level.rs`, `evm.rs`.

Nghiệm thu: test CPM bằng số tính tay (bắt đầu muộn đẩy task sau, task tách cộng dồn đúng); đối chiếu một tệp MSPDI mẫu xuất từ Project.

### Pha 14 — phát hành macOS (S–M, song song Pha 13)

- Ký số Developer ID, hardened runtime, notarize và staple qua `tauri-action` với secrets Apple trong CI.
- Bản universal (`--target universal-apple-darwin`).
- `bundle.fileAssociations` cho `.pdproj` với UTI riêng; xử lý `RunEvent::Opened` qua `confirm_discard`; danh sách "Mở gần đây".
- CSP thay cho `csp: null`, cho phép `data:` và `blob:` mà jsPDF cần.
- Sửa metadata `src-tauri/Cargo.toml` (`description`, `authors`).

Nghiệm thu: CI chạy `codesign --verify --strict`, `spctl -a -vv`, `xcrun stapler validate`, `lipo -archs` (có x86_64 và arm64); bấm đúp tệp `.pdproj` mở được và vẫn hỏi lưu.

Rủi ro: cần tài khoản Apple Developer; CSP có thể làm hỏng xuất PDF nếu thiếu nguồn.

### Pha 15 — MSPDI đầy đủ (M, cần Pha 13)

- Đọc/ghi `Deadline`, chi phí thực tế, `WorkWeeks` của lịch, và tiến độ theo nghĩa mới của Pha 13.
- Nghiệm thu: test xuất rồi nhập lại với tệp từ Project 2019/365.

### Pha 16 — leveling (M–L, cần Pha 12)

- Quá tải tính theo giờ trùng nhau trong ngày, không cộng đơn vị của mọi task chạm cùng ngày.
- Không chạy lại `schedule()` toàn bộ mỗi vòng.
- Lưu độ trễ cân bằng thành giá trị cụ thể trong log, để replay không chạy lại heuristic.
- Nghiệm thu: hai task cùng ngày nhưng khác giờ không bị báo quá tải; bench 500 task dưới 1 giây.

### Pha 17 — UX ngang MS Project (L, tách 2–3 PR)

- Copy/paste và điền xuống (fill-down) task.
- Tách `App.tsx` (hơn 1.100 dòng); thêm test cho `GanttChart`, `ganttPdf`, `networkPdf`, `App`.
- Gantt dùng được bằng bàn phím; nút có kiểu focus.
- Kéo riêng từng đoạn của task tách.
- Sơ đồ mạng giữ liên kết gắn vào summary khi ẩn summary (chuyển xuống task con).
- Thanh công cụ chia nhóm, có nút Hoàn tác/Làm lại; tiêu đề cửa sổ là tên dự án kèm dấu đã sửa; thanh trạng thái chỉ hiện tên tệp.
- View Sử dụng chỉ tô quá tải ở ô có giờ; định dạng số thống nhất; bớt biểu tượng lịch lặp lại ở mỗi dòng.
- Màu thành token CSS rồi mới làm giao diện tối; xóa `src/App.css` thừa từ template.

### Ngoài phạm vi

Reader tệp nhị phân `.mpp` (xem pha 5), nhiều người dùng, server, công thức trường tùy chỉnh, ký số bản Windows/Linux, dựng lại chuỗi lặp khi nhập MSPDI. Giao diện chỉ có tiếng Việt; đa ngôn ngữ cần quyết định riêng trước khi đưa vào lộ trình.
