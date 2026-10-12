import { defineMessages } from "../../i18n";

export const splitSectionMessages = defineMessages(
  {
    title: "Tách công việc",
    segment: (index: number, start: string, finish: string) => `Đoạn ${index}: ${start} – ${finish}`,
    gap: (duration: string) => `Ngắt ${duration} làm việc`,
    join: (next: number) => `Nối với đoạn ${next}`,
    split: "Tách…",
    joinAll: "Nối tất cả các đoạn",
  },
  {
    title: "Split task",
    segment: (index: number, start: string, finish: string) => `Part ${index}: ${start} – ${finish}`,
    gap: (duration: string) => `Gap of ${duration} of working time`,
    join: (next: number) => `Join with part ${next}`,
    split: "Split…",
    joinAll: "Join all parts",
  },
);
