import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { I18nProvider, setCurrentLang } from "../i18n";
import { at } from "../test/fixtures";
import { DateKeyField, DateTimeField, TimeField } from "./DateTimeField";

const START = at(2026, 1, 5, 8, 30);

function Controlled({ initial, optional, onCommit }: { initial: number | null; optional?: boolean; onCommit: (ms: number | null) => void }) {
  const [value, setValue] = useState(initial);
  return (
    <DateTimeField
      ariaLabel="Ngày"
      optional={optional}
      value={value}
      onCommit={(ms) => {
        onCommit(ms);
        setValue(ms);
      }}
    />
  );
}

function setup(initial: number | null = START, optional = false) {
  const onCommit = vi.fn();
  const user = userEvent.setup();
  render(<Controlled initial={initial} optional={optional} onCommit={onCommit} />);
  const input = screen.getByLabelText("Ngày") as HTMLInputElement;
  return { user, onCommit, input };
}

describe("DateTimeField popover", () => {
  it("shows the value and opens a calendar on the month of the value", async () => {
    const { user, input } = setup();
    expect(input).toHaveValue("05/01/2026 08:30");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();

    await user.click(input);

    const dialog = screen.getByRole("dialog", { name: "Chọn ngày" });
    expect(input).toHaveAttribute("aria-expanded", "true");
    expect(within(dialog).getByText("Tháng 1, 2026")).toBeInTheDocument();
    expect(within(dialog).getByRole("gridcell", { name: "05/01/2026" })).toHaveAttribute("aria-selected", "true");
    expect(within(dialog).getAllByRole("gridcell")).toHaveLength(42);
  });

  it("opens from the calendar button and closes on Escape", async () => {
    const { user, input } = setup();
    await user.click(screen.getByRole("button", { name: "Mở lịch chọn ngày" }));
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(input).toHaveFocus();
  });

  it("closes when clicking outside", async () => {
    const { user } = setup();
    await user.click(screen.getByRole("button", { name: "Mở lịch chọn ngày" }));
    await user.click(document.body);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("picks a day, keeps the time and commits on Chọn", async () => {
    const { user, input, onCommit } = setup();
    await user.click(input);
    await user.click(screen.getByRole("gridcell", { name: "14/01/2026" }));
    expect(onCommit).not.toHaveBeenCalled();
    expect(screen.getByRole("gridcell", { name: "14/01/2026" })).toHaveAttribute("aria-selected", "true");

    await user.click(screen.getByRole("button", { name: "Chọn" }));

    expect(onCommit).toHaveBeenCalledExactlyOnceWith(at(2026, 1, 14, 8, 30));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(input).toHaveValue("14/01/2026 08:30");
  });

  it("commits immediately on double-click", async () => {
    const { user, input, onCommit } = setup();
    await user.click(input);
    await user.dblClick(screen.getByRole("gridcell", { name: "20/01/2026" }));
    expect(onCommit).toHaveBeenLastCalledWith(at(2026, 1, 20, 8, 30));
  });

  it("changes hour and minute with the selects", async () => {
    const { user, input, onCommit } = setup();
    await user.click(input);
    await user.selectOptions(screen.getByRole("combobox", { name: "Giờ" }), "14");
    await user.selectOptions(screen.getByRole("combobox", { name: "Phút" }), "45");
    await user.click(screen.getByRole("button", { name: "Chọn" }));
    expect(onCommit).toHaveBeenCalledExactlyOnceWith(at(2026, 1, 5, 14, 45));
  });

  it("navigates months", async () => {
    const { user, input } = setup();
    await user.click(input);
    await user.click(screen.getByRole("button", { name: "Tháng trước" }));
    expect(screen.getByText("Tháng 12, 2025")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Tháng sau" }));
    await user.click(screen.getByRole("button", { name: "Tháng sau" }));
    expect(screen.getByText("Tháng 2, 2026")).toBeInTheDocument();
  });

  it("Hủy closes without committing", async () => {
    const { user, input, onCommit } = setup();
    await user.click(input);
    await user.click(screen.getByRole("gridcell", { name: "14/01/2026" }));
    await user.click(screen.getByRole("button", { name: "Hủy" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    await user.tab();
    expect(onCommit).not.toHaveBeenCalled();
    expect(input).toHaveValue("05/01/2026 08:30");
  });
});

describe("DateTimeField keyboard typing", () => {
  it("commits typed text on Enter", async () => {
    const { user, input, onCommit } = setup();
    await user.click(input);
    await user.clear(input);
    await user.type(input, "20/02/2026 09:15{Enter}");
    expect(onCommit).toHaveBeenCalledExactlyOnceWith(at(2026, 2, 20, 9, 15));
    expect(input).not.toHaveFocus();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("commits typed text on blur and defaults a date-only entry to 08:00", async () => {
    const { user, input, onCommit } = setup();
    await user.click(input);
    await user.clear(input);
    await user.type(input, "3/3/2026");
    await user.tab();
    expect(onCommit).toHaveBeenCalledExactlyOnceWith(at(2026, 3, 3, 8, 0));
  });

  it("seeds the calendar from what is being typed", async () => {
    const { user, input } = setup();
    await user.click(input);
    await user.clear(input);
    await user.type(input, "10/06/2026");
    await user.keyboard("{Escape}");
    await user.keyboard("{ArrowDown}");
    expect(screen.getByText("Tháng 6, 2026")).toBeInTheDocument();
    expect(screen.getByRole("gridcell", { name: "10/06/2026" })).toHaveAttribute("aria-selected", "true");
  });

  it("ignores invalid text and shows the old value again", async () => {
    const { user, input, onCommit } = setup();
    await user.click(input);
    await user.clear(input);
    await user.type(input, "31/02/2026{Enter}");
    expect(onCommit).not.toHaveBeenCalled();
    expect(input).toHaveValue("05/01/2026 08:30");
  });

  it("does not commit when the value is unchanged", async () => {
    const { user, input, onCommit } = setup();
    await user.click(input);
    await user.keyboard("{Enter}");
    expect(onCommit).not.toHaveBeenCalled();
  });

  it("ignores empty text on a required field", async () => {
    const { user, input, onCommit } = setup(START, false);
    await user.click(input);
    await user.clear(input);
    await user.tab();
    expect(onCommit).not.toHaveBeenCalled();
    expect(input).toHaveValue("05/01/2026 08:30");
  });
});

describe("DateTimeField optional values", () => {
  it("has no clear button for required fields", () => {
    setup(START, false);
    expect(screen.queryByRole("button", { name: "Xóa ngày" })).not.toBeInTheDocument();
  });

  it("clears with the × button", async () => {
    const { user, input, onCommit } = setup(START, true);
    await user.click(screen.getByRole("button", { name: "Xóa ngày" }));
    expect(onCommit).toHaveBeenCalledExactlyOnceWith(null);
    expect(input).toHaveValue("");
    expect(screen.queryByRole("button", { name: "Xóa ngày" })).not.toBeInTheDocument();
  });

  it("clears by erasing the text", async () => {
    const { user, input, onCommit } = setup(START, true);
    await user.click(input);
    await user.clear(input);
    await user.tab();
    expect(onCommit).toHaveBeenCalledExactlyOnceWith(null);
  });

  it("clears from the popover", async () => {
    const { user, input, onCommit } = setup(START, true);
    await user.click(input);
    await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Xóa" }));
    expect(onCommit).toHaveBeenCalledExactlyOnceWith(null);
  });

  it("an empty optional field opens on today and can be set", async () => {
    vi.useFakeTimers({ toFake: ["Date"] });
    vi.setSystemTime(new Date(2026, 9, 10, 12, 0));
    try {
      const { user, input, onCommit } = setup(null, true);
      expect(input).toHaveValue("");
      await user.click(input);
      expect(screen.getByText("Tháng 10, 2026")).toBeInTheDocument();
      const today = screen.getByRole("gridcell", { name: "10/10/2026" });
      expect(today).toHaveClass("today");
      await user.click(screen.getByRole("button", { name: "Chọn" }));
      expect(onCommit).toHaveBeenCalledExactlyOnceWith(at(2026, 10, 10, 8, 0));
    } finally {
      vi.useRealTimers();
    }
  });
});

describe("DateTimeField disabled", () => {
  it("does not open", async () => {
    const user = userEvent.setup();
    render(<DateTimeField ariaLabel="Ngày" disabled value={START} onCommit={vi.fn()} />);
    await user.click(screen.getByLabelText("Ngày"));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Mở lịch chọn ngày" })).toBeDisabled();
  });
});

describe("DateKeyField and TimeField", () => {
  it("date mode commits a YYYY-MM-DD key with a single click", async () => {
    const user = userEvent.setup();
    const onCommit = vi.fn();
    render(<DateKeyField ariaLabel="Ngày nghỉ" value="2026-04-30" onCommit={onCommit} />);
    const input = screen.getByLabelText("Ngày nghỉ");
    expect(input).toHaveValue("30/04/2026");
    await user.click(input);
    expect(screen.queryByRole("combobox", { name: "Giờ" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("gridcell", { name: "01/05/2026" }));
    expect(onCommit).toHaveBeenCalledExactlyOnceWith("2026-05-01");
  });

  it("date mode shows a placeholder for an empty key", () => {
    render(<DateKeyField ariaLabel="Ngày nghỉ" value="" onCommit={vi.fn()} />);
    expect(screen.getByLabelText("Ngày nghỉ")).toHaveAttribute("placeholder", "dd/mm/yyyy");
  });

  it("time mode allows 24:00 and disables minutes at 24", async () => {
    const user = userEvent.setup();
    const onCommit = vi.fn();
    render(<TimeField ariaLabel="Giờ kết thúc" minute={17 * 60} onCommit={onCommit} />);
    const input = screen.getByLabelText("Giờ kết thúc");
    expect(input).toHaveValue("17:00");
    await user.click(input);
    expect(screen.getByRole("dialog", { name: "Chọn giờ" })).toBeInTheDocument();
    await user.selectOptions(screen.getByRole("combobox", { name: "Giờ" }), "24");
    expect(screen.getByRole("combobox", { name: "Phút" })).toBeDisabled();
    await user.click(screen.getByRole("button", { name: "Chọn" }));
    expect(onCommit).toHaveBeenCalledExactlyOnceWith(24 * 60);
  });

  it("time mode accepts typed times", async () => {
    const user = userEvent.setup();
    const onCommit = vi.fn();
    render(<TimeField ariaLabel="Giờ" minute={8 * 60} onCommit={onCommit} />);
    const input = screen.getByRole("textbox", { name: "Giờ" });
    await user.click(input);
    await user.clear(input);
    await user.type(input, "13h30{Enter}");
    expect(onCommit).toHaveBeenCalledExactlyOnceWith(13 * 60 + 30);
  });
});

describe("DateTimeField in English", () => {
  afterEach(() => setCurrentLang("vi"));

  it("shows English month, weekdays and buttons, and ISO dates", async () => {
    const user = userEvent.setup();
    render(
      <I18nProvider lang="en">
        <DateTimeField ariaLabel="Start" value={START} onCommit={() => {}} />
      </I18nProvider>,
    );
    expect(screen.getByLabelText("Start")).toHaveValue("2026-01-05 08:30");
    await user.click(screen.getByRole("button", { name: "Open date picker" }));
    const dialog = screen.getByRole("dialog", { name: "Choose date" });
    expect(within(dialog).getByText("January 2026")).toBeInTheDocument();
    expect(within(dialog).getAllByRole("columnheader").map((cell) => cell.textContent)).toEqual([
      "Mon",
      "Tue",
      "Wed",
      "Thu",
      "Fri",
      "Sat",
      "Sun",
    ]);
    expect(within(dialog).getByRole("gridcell", { name: "2026-01-05" })).toHaveAttribute("aria-selected", "true");
    expect(within(dialog).getByRole("button", { name: "Today" })).toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "Cancel" })).toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "OK" })).toBeInTheDocument();
    expect(within(dialog).getByRole("combobox", { name: "Hour" })).toBeInTheDocument();
    expect(within(dialog).getByRole("combobox", { name: "Minute" })).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Previous month" }));
    expect(within(dialog).getByText("December 2025")).toBeInTheDocument();
  });

  it("uses an ISO placeholder for date keys and English labels for times and clearing", () => {
    render(
      <I18nProvider lang="en">
        <DateKeyField ariaLabel="Date" value="" onCommit={() => {}} />
        <TimeField ariaLabel="Time" minute={480} onCommit={() => {}} />
        <DateTimeField ariaLabel="Due" optional value={START} onCommit={() => {}} />
      </I18nProvider>,
    );
    expect(screen.getByLabelText("Date")).toHaveAttribute("placeholder", "yyyy-mm-dd");
    expect(screen.getByRole("button", { name: "Choose time" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Clear date" })).toBeInTheDocument();
  });

  it("keeps the Vietnamese placeholder in Vietnamese", () => {
    render(<DateKeyField ariaLabel="Ngày" value="" onCommit={() => {}} />);
    expect(screen.getByLabelText("Ngày")).toHaveAttribute("placeholder", "dd/mm/yyyy");
  });
});
