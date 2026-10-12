import { render, screen } from "@testing-library/react";
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
});
