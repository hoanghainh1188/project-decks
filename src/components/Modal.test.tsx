import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { StrictMode, useState } from "react";
import { describe, expect, it, vi } from "vitest";

import { Modal } from "./Modal";

interface HarnessProps {
  onClose?: () => void;
  onCommit?: (value: string) => void;
  autoFocus?: boolean;
}

function Harness({ onClose, onCommit, autoFocus = false }: HarnessProps) {
  const [open, setOpen] = useState(false);
  const close = () => {
    onClose?.();
    setOpen(false);
  };
  return (
    <>
      <button type="button" onClick={() => setOpen(true)}>Mở</button>
      {open && (
        <Modal title="Hộp thử" onClose={onClose ? close : undefined}>
          <input
            aria-label="Tên"
            autoFocus={autoFocus}
            onKeyDown={(event) => event.key === "Enter" && event.currentTarget.blur()}
            onBlur={(event) => onCommit?.(event.target.value)}
          />
          <button type="button" onClick={close}>Đóng</button>
        </Modal>
      )}
    </>
  );
}

describe("Modal", () => {
  it("is a named modal dialog that takes focus and gives it back", async () => {
    const user = userEvent.setup();
    render(<Harness onClose={vi.fn()} />);
    const trigger = screen.getByRole("button", { name: "Mở" });
    await user.click(trigger);
    const dialog = screen.getByRole("dialog", { name: "Hộp thử" });
    expect(dialog).toHaveAttribute("aria-modal", "true");
    expect(dialog).toHaveFocus();
    await user.click(screen.getByRole("button", { name: "Đóng" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(trigger).toHaveFocus();
  });

  it("leaves focus with a child that took it on mount", async () => {
    const user = userEvent.setup();
    render(<Harness autoFocus />);
    await user.click(screen.getByRole("button", { name: "Mở" }));
    expect(screen.getByRole("textbox", { name: "Tên" })).toHaveFocus();
  });

  it("keeps Tab inside the dialog", async () => {
    const user = userEvent.setup();
    render(<Harness autoFocus />);
    await user.click(screen.getByRole("button", { name: "Mở" }));
    const name = screen.getByRole("textbox", { name: "Tên" });
    const close = screen.getByRole("button", { name: "Đóng" });
    await user.tab();
    expect(close).toHaveFocus();
    await user.tab();
    expect(name).toHaveFocus();
    await user.tab({ shift: true });
    expect(close).toHaveFocus();
  });

  it("closes on Escape and on the backdrop only when it has onClose", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    const { unmount } = render(<Harness onClose={onClose} />);
    await user.click(screen.getByRole("button", { name: "Mở" }));
    await user.keyboard("{Escape}");
    expect(onClose).toHaveBeenCalledOnce();
    await user.click(screen.getByRole("button", { name: "Mở" }));
    fireEvent.mouseDown(screen.getByRole("dialog").parentElement as HTMLElement);
    expect(onClose).toHaveBeenCalledTimes(2);
    unmount();

    render(<Harness />);
    await user.click(screen.getByRole("button", { name: "Mở" }));
    await user.keyboard("{Escape}");
    fireEvent.mouseDown(screen.getByRole("dialog").parentElement as HTMLElement);
    expect(screen.getByRole("dialog")).toBeInTheDocument();
  });

  it("still closes on Escape after Enter moved focus out of the field", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    render(<Harness onClose={onClose} autoFocus />);
    await user.click(screen.getByRole("button", { name: "Mở" }));
    await user.keyboard("Lan{Enter}");
    expect(document.body).toHaveFocus();
    await user.tab();
    expect(screen.getByRole("button", { name: "Đóng hộp thoại" })).toHaveFocus();
    await user.click(document.body);
    await user.keyboard("{Escape}");
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("commits the field being edited before Escape closes", async () => {
    const user = userEvent.setup();
    const onCommit = vi.fn();
    render(<Harness onClose={vi.fn()} onCommit={onCommit} autoFocus />);
    await user.click(screen.getByRole("button", { name: "Mở" }));
    await user.keyboard("Dự án mới{Escape}");
    expect(onCommit).toHaveBeenCalledWith("Dự án mới");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("keeps a child's focus under StrictMode", async () => {
    const user = userEvent.setup();
    render(
      <StrictMode>
        <Harness autoFocus />
      </StrictMode>,
    );
    await user.click(screen.getByRole("button", { name: "Mở" }));
    expect(screen.getByRole("textbox", { name: "Tên" })).toHaveFocus();
  });

  it("has a close button in the header and keeps the footer apart from the scrolling body", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    render(
      <Modal title="Có chân" onClose={onClose} footer={<button type="button">Lưu</button>}>
        <p>Nội dung</p>
      </Modal>,
    );
    const dialog = screen.getByRole("dialog", { name: "Có chân" });
    expect(dialog.querySelector(".dialog-body")).toHaveTextContent("Nội dung");
    expect(dialog.querySelector(".dialog-foot")).toContainElement(screen.getByRole("button", { name: "Lưu" }));
    await user.click(screen.getByRole("button", { name: "Đóng hộp thoại" }));
    expect(onClose).toHaveBeenCalledOnce();
  });
});
