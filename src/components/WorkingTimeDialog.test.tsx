import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { makeProject, standardCalendar } from "../test/fixtures";
import { WorkingTimeDialog } from "./WorkingTimeDialog";

function renderDialog(initialId: number) {
  const props = {
    project: makeProject({ calendars: [standardCalendar(1, "Chuẩn"), standardCalendar(2, "Ca đêm")] }),
    initialId,
    changed: true,
    onSave: vi.fn(),
    onCancel: vi.fn(),
    send: vi.fn().mockResolvedValue(undefined),
    onCreateCalendar: vi.fn().mockResolvedValue(2),
  };
  render(<WorkingTimeDialog {...props} />);
  return { ...props, user: userEvent.setup() };
}

describe("WorkingTimeDialog", () => {
  it("opens on the calendar asked for, marks the project calendar and switches", async () => {
    const { user } = renderDialog(2);
    const select = screen.getByLabelText("Lịch");
    expect(select).toHaveValue("2");
    expect(screen.getByRole("option", { name: "Chuẩn (lịch dự án)" })).toBeInTheDocument();
    expect(screen.getByLabelText("Tên lịch")).toHaveValue("Ca đêm");
    await user.selectOptions(select, "1");
    expect(screen.getByLabelText("Tên lịch")).toHaveValue("Chuẩn");
  });

  it("is a transaction: Lưu keeps, Hủy rolls back, and a new calendar is shown", async () => {
    const { user, onSave, onCancel, onCreateCalendar } = renderDialog(1);
    expect(screen.getByText("Đang xem trước thay đổi trên lịch")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Tạo lịch" }));
    expect(onCreateCalendar).toHaveBeenCalled();
    expect(screen.getByLabelText("Lịch")).toHaveValue("2");
    await user.click(screen.getByRole("button", { name: "Hủy" }));
    expect(onCancel).toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Lưu" }));
    expect(onSave).toHaveBeenCalled();
  });
});
