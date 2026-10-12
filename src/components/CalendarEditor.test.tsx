import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import { I18nProvider, setCurrentLang } from "../i18n";
import { standardCalendar } from "../test/fixtures";
import type { Calendar } from "../types";
import { CalendarEditor } from "./CalendarEditor";

function calendar(): Calendar {
  return {
    ...standardCalendar(1, "Tiêu chuẩn"),
    exceptions: {
      "2026-01-01": [],
      "2026-01-02": [],
      "2026-01-10": [{ start_minute: 8 * 60, end_minute: 12 * 60 }],
    },
  };
}

describe("CalendarEditor", () => {
  afterEach(() => setCurrentLang("vi"));

  it("shows Vietnamese by default", () => {
    render(<CalendarEditor calendar={calendar()} send={vi.fn(async () => undefined)} />);
    expect(screen.getByRole("heading", { name: "Tuần làm việc" })).toBeInTheDocument();
    expect(screen.getByLabelText("Thứ Hai")).toBeChecked();
    expect(screen.getByLabelText("Chủ Nhật")).not.toBeChecked();
    expect(screen.getByText("01/01/2026 – 02/01/2026 · Nghỉ cả ngày")).toBeInTheDocument();
  });

  it("shows English labels, weekdays and exceptions", async () => {
    const user = userEvent.setup();
    render(
      <I18nProvider lang="en">
        <CalendarEditor calendar={calendar()} send={vi.fn(async () => undefined)} />
      </I18nProvider>,
    );
    expect(screen.getByLabelText("Calendar name")).toHaveValue("Tiêu chuẩn");
    expect(screen.getByRole("heading", { name: "Work week" })).toBeInTheDocument();
    expect(screen.getByLabelText("Monday")).toBeChecked();
    expect(screen.getByLabelText("Sunday")).not.toBeChecked();
    expect(screen.getAllByRole("button", { name: "Add range" }).length).toBeGreaterThan(0);
    expect(screen.getAllByLabelText("Start time").length).toBeGreaterThan(0);
    expect(screen.getByRole("heading", { name: "Nonworking days and exceptions" })).toBeInTheDocument();
    expect(screen.getByText("2026-01-01 – 2026-01-02 · Nonworking all day")).toBeInTheDocument();
    expect(screen.getByText("2026-01-10 · 08:00–12:00")).toBeInTheDocument();
    expect(screen.getByLabelText("Date")).toHaveAttribute("placeholder", "yyyy-mm-dd");

    await user.selectOptions(screen.getByDisplayValue("One day"), "range");
    expect(screen.getByLabelText("From date")).toBeInTheDocument();
    expect(screen.getByLabelText("To date")).toBeInTheDocument();
    await user.selectOptions(screen.getByDisplayValue("Nonworking all day"), "hours");
    expect(screen.getByRole("option", { name: "Custom hours" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Add exception" })).toBeDisabled();
  });

  it("says when there are no exceptions", () => {
    render(
      <I18nProvider lang="en">
        <CalendarEditor calendar={standardCalendar(1, "Std")} send={vi.fn(async () => undefined)} />
      </I18nProvider>,
    );
    expect(screen.getByText("No nonworking days or exceptions yet.")).toBeInTheDocument();
  });

  it("renames the calendar, and keeps the old name when the field is emptied", async () => {
    const user = userEvent.setup();
    const send = vi.fn(async () => undefined);
    render(<CalendarEditor calendar={calendar()} send={send} />);
    const name = screen.getByLabelText("Tên lịch");
    await user.clear(name);
    await user.tab();
    expect(name).toHaveValue("Tiêu chuẩn");
    expect(send).not.toHaveBeenCalled();
    await user.type(name, " ca đêm");
    await user.tab();
    expect(send).toHaveBeenCalledExactlyOnceWith({ type: "rename_calendar", calendar_id: 1, name: "Tiêu chuẩn ca đêm" });
  });

  it("turns weekdays on and off as a whole week", async () => {
    const user = userEvent.setup();
    const send = vi.fn(async () => undefined);
    const base = calendar();
    render(<CalendarEditor calendar={base} send={send} />);
    await user.click(screen.getByLabelText("Thứ Hai"));
    expect(send).toHaveBeenLastCalledWith({
      type: "set_calendar_week",
      calendar_id: 1,
      week: [[], ...base.week.slice(1)],
    });
    await user.click(screen.getByLabelText("Thứ Bảy"));
    const week = (send.mock.lastCall as unknown as [{ week: Calendar["week"] }])[0].week;
    expect(week[5]).toEqual(base.week[0]);
    expect(week[0]).toEqual(base.week[0]);
  });

  it("adds a nonworking day and removes an exception", async () => {
    const user = userEvent.setup();
    const send = vi.fn(async () => ({}) as never);
    render(
      <I18nProvider lang="en">
        <CalendarEditor calendar={calendar()} send={send} />
      </I18nProvider>,
    );
    await user.type(screen.getByLabelText("Date"), "2026-01-15");
    await user.tab();
    await user.click(screen.getByRole("button", { name: "Add exception" }));
    expect(send).toHaveBeenCalledWith({
      type: "set_calendar_exceptions",
      calendar_id: 1,
      from: "2026-01-15",
      to: "2026-01-15",
      ranges: [],
    });
    const holiday = screen.getByText("2026-01-01 – 2026-01-02 · Nonworking all day").closest("li") as HTMLElement;
    await user.click(within(holiday).getByRole("button", { name: "Remove" }));
    expect(send).toHaveBeenLastCalledWith({
      type: "clear_calendar_exceptions",
      calendar_id: 1,
      from: "2026-01-01",
      to: "2026-01-02",
    });
  });
});
