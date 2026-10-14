import { defineMessages } from "../../i18n";

export const assignmentEditorMessages = defineMessages(
  {
    assign: "Gán nguồn lực",
    resourceFallback: (uid: number) => `NL ${uid}`,
    units: "Đơn vị",
    unitsOf: (name: string) => `Đơn vị ${name}`,
    remove: "Bỏ",
    newUnits: "Đơn vị gán",
    unitsTip: "Phần mỗi ngày làm việc: 1 là cả ngày, 0,5 nửa ngày; gõ được “2h/ngày” (tính trên ngày 8 giờ)",
    equals: (perDay: string) => `= ${perDay}`,
    choose: "Chọn…",
    actualCost: "Thực tế",
    actualCostOf: (name: string) => `Chi phí thực tế ${name}`,
  },
  {
    assign: "Assign resources",
    resourceFallback: (uid: number) => `Resource ${uid}`,
    units: "Units",
    unitsOf: (name: string) => `Units for ${name}`,
    remove: "Remove",
    newUnits: "Units to assign",
    unitsTip: "The share of each working day: 1 is the whole day, 0.5 half; you can type “2h/day” (on an 8-hour day)",
    equals: (perDay: string) => `= ${perDay}`,
    choose: "Choose…",
    actualCost: "Actual",
    actualCostOf: (name: string) => `Actual cost for ${name}`,
  },
);
