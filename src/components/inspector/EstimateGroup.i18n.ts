import { defineMessages } from "../../i18n";

export const estimateGroupMessages = defineMessages(
  {
    legend: "Ước lượng",
    low: "Effort thấp",
    high: "Effort cao",
    note: "Khoảng effort cho dự báo ngày kết thúc; để trống cả hai nếu chắc chắn.",
    needBoth: "Cần cả mức thấp và mức cao (hoặc để trống cả hai)",
    backwards: "Mức thấp lớn hơn mức cao",
    unreadable: "Không đọc được thời lượng",
    outside: (amount: string) => `Effort hiện tại nằm ngoài khoảng (${amount}); dự báo lấy mức gần nhất làm giá trị khả dĩ nhất.`,
  },
  {
    legend: "Estimate",
    low: "Low effort",
    high: "High effort",
    note: "The effort range for the finish forecast; leave both empty when the effort is certain.",
    needBoth: "Enter both a low and a high amount (or leave both empty)",
    backwards: "The low amount is larger than the high one",
    unreadable: "Could not read the amount",
    outside: (amount: string) => `Today's effort lies outside the range (${amount}); the forecast takes the nearest end as the likely value.`,
  },
);
