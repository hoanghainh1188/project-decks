import { defineMessages } from "../i18n";

export const taskEditDialogMessages = defineMessages(
  {
    title: "Sửa công việc",
    close: "Đóng",
    name: "Tên",
    duration: "Thời lượng",
    start: "Bắt đầu",
    finish: "Kết thúc",
    percentComplete: "% hoàn thành",
    predecessors: "Tiền nhiệm",
    predecessorsPlaceholder: "VD: 2, 3SS+1d",
    interrupts: "Chen ngang",
    interruptsNote: "Khi cân bằng, việc này được làm trước việc đang dở có thứ tự thấp hơn của cùng người.",
  },
  {
    title: "Edit task",
    close: "Close",
    name: "Name",
    duration: "Duration",
    start: "Start",
    finish: "Finish",
    percentComplete: "% Complete",
    predecessors: "Predecessors",
    predecessorsPlaceholder: "e.g. 2, 3SS+1d",
    interrupts: "Interrupts",
    interruptsNote: "When leveling, this task goes ahead of work in progress ranked lower on the same people.",
  },
);
