import { defineMessages } from "../../i18n";

export const assignmentEditorMessages = defineMessages(
  {
    assign: "Gán nguồn lực",
    resourceFallback: (uid: number) => `NL ${uid}`,
    units: "Đơn vị",
    unitsOf: (name: string) => `Đơn vị ${name}`,
    remove: "Bỏ",
    newUnits: "Đơn vị gán",
    choose: "Chọn…",
  },
  {
    assign: "Assign resources",
    resourceFallback: (uid: number) => `Resource ${uid}`,
    units: "Units",
    unitsOf: (name: string) => `Units for ${name}`,
    remove: "Remove",
    newUnits: "Units to assign",
    choose: "Choose…",
  },
);
