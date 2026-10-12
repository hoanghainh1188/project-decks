import type { LucideIcon } from "lucide-react";
import type { ButtonHTMLAttributes, Ref } from "react";

/**
 * primary: the action the dialog is for (Tách, Tạo chuỗi, Đặt); one per dialog.
 * secondary: closing, cancelling and small edits.
 * danger: removes something.
 */
export type ButtonVariant = "primary" | "secondary" | "danger";

interface Props extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  /** Decorative; the text stays the accessible name. */
  Icon?: LucideIcon;
  ref?: Ref<HTMLButtonElement>;
}

/** A dialog or panel button with an optional icon, styled by what it does. */
export function Button({ variant = "secondary", Icon, className, children, type = "button", ...rest }: Props) {
  return (
    <button type={type} className={["btn", `btn-${variant}`, className].filter(Boolean).join(" ")} {...rest}>
      {Icon && <Icon aria-hidden="true" size={14} strokeWidth={2} />}
      {children}
    </button>
  );
}
