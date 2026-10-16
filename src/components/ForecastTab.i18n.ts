import { defineMessages } from "../i18n";

export const forecastTabMessages = defineMessages(
  {
    intro:
      "Dự báo ngày kết thúc lấy mẫu effort của mỗi việc trong khoảng Effort thấp–cao của nó. Các việc thường trễ cùng nhau (cùng một mảng, cùng một đội ước lượng lạc quan), nên dự báo cho chúng tương quan với nhau.",
    field: "Tương quan theo",
    noField: "Không theo trường nào",
    fieldNote: "Việc cùng giá trị của trường này (văn bản hoặc danh sách chọn) trễ hoặc sớm cùng nhau.",
    group: "Mức trong nhóm",
    groupNeedsField: "Chọn một trường ở trên để dùng mức trong nhóm.",
    common: "Mức chung",
    commonNote: "Phần mọi việc cùng chịu, như cả đội cùng ước lượng lạc quan.",
    none: "Không",
    low: "Thấp",
    medium: "Vừa",
    high: "Cao",
  },
  {
    intro:
      "The finish forecast samples each task's effort within its low–high effort range. Tasks often slip together (the same area, the same team estimating optimistically), so the forecast correlates them.",
    field: "Correlate by",
    noField: "No field",
    fieldNote: "Tasks with the same value of this field (text or choice) run late or early together.",
    group: "Within a group",
    groupNeedsField: "Choose a field above to use the strength within a group.",
    common: "Across all tasks",
    commonNote: "The part every task shares, such as a whole team estimating optimistically.",
    none: "None",
    low: "Low",
    medium: "Medium",
    high: "High",
  },
);
