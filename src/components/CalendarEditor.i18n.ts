import { defineMessages } from "../i18n";

const VI_WEEKDAYS = ["Thứ Hai", "Thứ Ba", "Thứ Tư", "Thứ Năm", "Thứ Sáu", "Thứ Bảy", "Chủ Nhật"];
const EN_WEEKDAYS = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];

export const calendarEditorMessages = defineMessages(
  {
    /** A day of the work week; `index` 0 is Monday. */
    weekday: (index: number) => VI_WEEKDAYS[index] ?? "",
    calendarName: "Tên lịch",
    workWeek: "Tuần làm việc",
    addRange: "Thêm khoảng",
    exceptionsHeading: "Ngày nghỉ và ngoại lệ",
    oneDay: "Một ngày",
    dateRange: "Từ–Đến",
    date: "Ngày",
    from: "Từ",
    fromDate: "Từ ngày",
    to: "Đến",
    toDate: "Đến ngày",
    nonworking: "Nghỉ cả ngày",
    customHours: "Giờ làm riêng",
    addException: "Thêm ngoại lệ",
    noExceptions: "Chưa có ngày nghỉ hay ngoại lệ.",
    remove: "Bỏ",
    startTime: "Giờ bắt đầu",
    finishTime: "Giờ kết thúc",
  },
  {
    weekday: (index: number) => EN_WEEKDAYS[index] ?? "",
    calendarName: "Calendar name",
    workWeek: "Work week",
    addRange: "Add range",
    exceptionsHeading: "Nonworking days and exceptions",
    oneDay: "One day",
    dateRange: "From–To",
    date: "Date",
    from: "From",
    fromDate: "From date",
    to: "To",
    toDate: "To date",
    nonworking: "Nonworking all day",
    customHours: "Custom hours",
    addException: "Add exception",
    noExceptions: "No nonworking days or exceptions yet.",
    remove: "Remove",
    startTime: "Start time",
    finishTime: "Finish time",
  },
);

/** Monday-first indexes into `calendar.week`. */
export const WEEK_INDEXES = [0, 1, 2, 3, 4, 5, 6] as const;
