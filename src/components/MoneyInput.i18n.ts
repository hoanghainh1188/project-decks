import { defineMessages } from "../i18n";

export const moneyInputMessages = defineMessages(
  {
    unreadable: "Không đọc được số tiền; giữ giá trị cũ. Ví dụ: 1.248.000 hoặc 65000,5.",
    required: "Cần một số tiền (0 nếu không có); giữ giá trị cũ.",
  },
  {
    unreadable: "Could not read the amount; the old value stays. For example 1,248,000 or 65000.5.",
    required: "An amount is needed (0 for none); the old value stays.",
  },
);
