import { defineMessages } from "../i18n";

export const fieldsTabMessages = defineMessages(
  {
    intro:
      "Trường tùy chỉnh là cột riêng của công việc, ví dụ khách hàng, mảng việc hay mức rủi ro. Dùng chúng để lọc và nhóm công việc.",
    none: "Dự án chưa có trường tùy chỉnh.",
    add: "Thêm trường",
    newField: (n: number) => `Trường ${n}`,
    limits: (text: number, number: number) =>
      `Tối đa ${text} trường chữ hoặc danh sách và ${number} trường số, như Microsoft Project.`,
    textLimit: (limit: number) => `Dự án đã có ${limit} trường chữ hoặc danh sách`,
    name: "Tên trường",
    kind: "Kiểu",
    text: "Chữ",
    number: "Số",
    choice: "Danh sách",
    used: (count: number) => (count === 0 ? "Chưa công việc nào có giá trị" : `${count} công việc có giá trị`),
    delete: "Xóa",
    deleteTip: "Xóa trường và mọi giá trị của nó (Hủy để lấy lại)",
    firstOption: "Lựa chọn 1",
    choiceN: (n: number) => `Lựa chọn ${n}`,
    removeChoice: (option: string) => `Bỏ lựa chọn “${option}”`,
    newChoice: "Lựa chọn mới",
    addChoice: "Thêm lựa chọn",
    removeChoiceTip: "Bỏ lựa chọn này; công việc đang có nó sẽ không còn giá trị",
  },
  {
    intro:
      "Custom fields are columns of your own for tasks, such as client, area of work or risk level. Use them to filter and group tasks.",
    none: "The project has no custom fields yet.",
    add: "Add field",
    newField: (n: number) => `Field ${n}`,
    limits: (text: number, number: number) =>
      `Up to ${text} text or list fields and ${number} number fields, as in Microsoft Project.`,
    textLimit: (limit: number) => `The project already has ${limit} text or list fields`,
    name: "Field name",
    kind: "Type",
    text: "Text",
    number: "Number",
    choice: "List",
    used: (count: number) => (count === 0 ? "No task has a value" : `${count} ${count === 1 ? "task has" : "tasks have"} a value`),
    delete: "Delete",
    deleteTip: "Delete the field and all its values (Cancel brings them back)",
    firstOption: "Choice 1",
    choiceN: (n: number) => `Choice ${n}`,
    removeChoice: (option: string) => `Remove choice “${option}”`,
    newChoice: "New choice",
    addChoice: "Add choice",
    removeChoiceTip: "Remove this choice; tasks that have it will have no value",
  },
);
