import { defineMessages } from "../../i18n";

export const forecastViewMessages = defineMessages(
  {
    working: "Đang tính dự báo…",
    refreshing: "Đang tính lại theo thay đổi mới…",
    noRanges:
      "Chưa việc nào có khoảng effort (nhóm Ước lượng hoặc cột ước lượng), nên các ngày dưới đây chưa có độ bất định.",
    summary: "Ngày xong dự án",
    planned: "Theo lịch hiện tại",
    leveled: "Khi đã cân bằng",
    chance: (percent: number) => `${percent}% khả năng xong trước`,
    vsLeveled: (days: number) => (days === 0 ? "cùng ngày khi đã cân bằng" : `${days > 0 ? "+" : ""}${days} ngày so với khi đã cân bằng`),
    vsPlanned: (days: number) => (days === 0 ? "cùng ngày với lịch hiện tại" : `${days > 0 ? "+" : ""}${days} ngày so với lịch hiện tại`),
    chart: (runs: number, p50: string, p90: string) =>
      `Phân bố ngày xong dự án qua ${runs} lần mô phỏng: 50% xong trước ${p50}, 90% xong trước ${p90}.`,
    byWeek: "Mỗi cột là một tuần.",
    byDay: "Mỗi cột là một ngày.",
    risky: "Việc cần chú ý",
    riskyNote: "Việc có hạn chót, khả năng kịp hạn thấp trước; rồi việc nằm trên đường găng ở ít nhất một nửa số lần.",
    none: "Không có việc nào có hạn chót hay thường nằm trên đường găng.",
    task: "Việc",
    deadline: "Hạn chót",
    p50: "Xong P50",
    p90: "Xong P90",
    onTime: "Kịp hạn",
    critical: "Găng",
    limits:
      "Giới hạn: thứ tự việc của từng người giữ như khi cân bằng; tương quan chỉ theo một trường và một mức chung; độ trễ liên kết không dao động.",
  },
  {
    working: "Working out the forecast…",
    refreshing: "Working it out again after the latest change…",
    noRanges:
      "No task has an effort range yet (the Estimate group or the estimate columns), so the dates below carry no uncertainty.",
    summary: "Project finish",
    planned: "As scheduled now",
    leveled: "Once leveled",
    chance: (percent: number) => `${percent}% chance to finish by`,
    vsLeveled: (days: number) => (days === 0 ? "the same day as once leveled" : `${days > 0 ? "+" : ""}${days} days against the leveled plan`),
    vsPlanned: (days: number) => (days === 0 ? "the same day as scheduled now" : `${days > 0 ? "+" : ""}${days} days against the schedule now`),
    chart: (runs: number, p50: string, p90: string) =>
      `Project finish over ${runs} simulated runs: 50% finish by ${p50}, 90% by ${p90}.`,
    byWeek: "Each bar is a week.",
    byDay: "Each bar is a day.",
    risky: "Tasks to watch",
    riskyNote: "Tasks with a deadline, least likely on time first; then tasks on the critical path in at least half the runs.",
    none: "No task has a deadline or is often on the critical path.",
    task: "Task",
    deadline: "Deadline",
    p50: "Finish P50",
    p90: "Finish P90",
    onTime: "On time",
    critical: "Critical",
    limits:
      "Limits: each person's order stays as leveling put it; correlation goes by one field and one common share; link lags do not vary.",
  },
);
