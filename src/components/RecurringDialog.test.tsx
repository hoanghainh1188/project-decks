import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { applyCommand } from "../api";
import { I18nProvider, setCurrentLang } from "../i18n";
import { fakeBackend } from "../test/fakeBackend";
import {
  at,
  DAY_MS,
  makeProject,
  makeResource,
  makeSnapshot,
  makeTask,
  standardCalendar,
} from "../test/fixtures";
import type { Project, RecurrenceSpec } from "../types";
import { RecurringDialog, recurringCommand } from "./RecurringDialog";

// Wednesday 07/01/2026 09:00; weekday index 2 (T4).
const START = at(2026, 1, 7, 9, 0);

function project(): Project {
  return makeProject({
    calendars: [standardCalendar(1, "Tiêu chuẩn"), standardCalendar(2, "Ca đêm")],
    resources: [makeResource({ uid: 10, name: "An" }), makeResource({ uid: 11, name: "" })],
  });
}

function renderDialog({
  initial = null,
  onSubmit = vi.fn(async () => true),
}: {
  initial?: RecurrenceSpec | null;
  onSubmit?: (spec: RecurrenceSpec) => Promise<unknown>;
} = {}) {
  const onClose = vi.fn();
  render(
    <RecurringDialog project={project()} initial={initial} defaultStart={START} onClose={onClose} onSubmit={onSubmit} />,
  );
  return { user: userEvent.setup(), onClose, onSubmit };
}

function lastSpec(onSubmit: (spec: RecurrenceSpec) => Promise<unknown>): RecurrenceSpec {
  const mock = vi.mocked(onSubmit);
  return mock.mock.calls[mock.mock.calls.length - 1][0];
}

async function replace(user: ReturnType<typeof userEvent.setup>, label: string, text: string) {
  const input = screen.getByLabelText(label);
  await user.clear(input);
  if (text) await user.type(input, text);
}

describe("RecurringDialog defaults", () => {
  it("submits a weekly series on the start weekday, 5 times", async () => {
    const { user, onSubmit, onClose } = renderDialog();
    expect(screen.getByRole("heading", { name: "Công việc lặp" })).toBeInTheDocument();
    expect(screen.getByLabelText("T4")).toBeChecked();
    expect(screen.getByLabelText("Bắt đầu chuỗi lặp")).toHaveValue("07/01/2026 09:00");
    expect(screen.getByLabelText("Ngày kết thúc chuỗi")).toBeDisabled();

    await user.click(screen.getByRole("button", { name: "Tạo chuỗi" }));

    expect(onSubmit).toHaveBeenCalledExactlyOnceWith({
      name: "Họp định kỳ",
      duration: 60,
      start: START,
      pattern: { kind: "weekly", every: 1, weekdays: [2] },
      end: { kind: "count", count: 5 },
      calendar_id: null,
      resources: [],
      background: false,
    });
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("stays open when the backend rejects the series", async () => {
    const { user, onClose } = renderDialog({ onSubmit: vi.fn(async () => undefined) });
    await user.click(screen.getByRole("button", { name: "Tạo chuỗi" }));
    expect(onClose).not.toHaveBeenCalled();
  });

  it("Hủy closes without submitting", async () => {
    const { user, onClose, onSubmit } = renderDialog();
    await user.click(screen.getByRole("button", { name: "Hủy" }));
    expect(onClose).toHaveBeenCalledOnce();
    expect(onSubmit).not.toHaveBeenCalled();
  });
});

describe("RecurringDialog validation", () => {
  it.each([
    ["Thời lượng mỗi lần", "abc", "Thời lượng không hợp lệ"],
    ["Thời lượng mỗi lần", "", "Thời lượng không hợp lệ"],
    ["Chu kỳ lặp", "0", "Chu kỳ lặp phải từ 1 trở lên"],
    ["Chu kỳ lặp", "x", "Chu kỳ lặp phải từ 1 trở lên"],
    ["Số lần lặp", "0", "Số lần lặp phải từ 1 đến 999"],
    ["Số lần lặp", "1000", "Số lần lặp phải từ 1 đến 999"],
  ])("%s = %j shows “%s”", async (label, text, message) => {
    const { user, onSubmit } = renderDialog();
    await replace(user, label, text);
    await user.click(screen.getByRole("button", { name: "Tạo chuỗi" }));
    expect(screen.getByText(message)).toBeInTheDocument();
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("requires at least one weekday for weekly series", async () => {
    const { user, onSubmit } = renderDialog();
    await user.click(screen.getByLabelText("T4"));
    await user.click(screen.getByRole("button", { name: "Tạo chuỗi" }));
    expect(screen.getByText("Hãy chọn ít nhất một thứ trong tuần")).toBeInTheDocument();
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it.each(["0", "32", "abc"])("rejects month day %j", async (day) => {
    const { user, onSubmit } = renderDialog();
    await user.click(screen.getByLabelText("Tháng"));
    await replace(user, "Ngày trong tháng", day);
    await user.click(screen.getByRole("button", { name: "Tạo chuỗi" }));
    expect(screen.getByText("Ngày trong tháng phải từ 1 đến 31")).toBeInTheDocument();
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("clears the message after a valid submit", async () => {
    const { user, onSubmit } = renderDialog();
    await replace(user, "Chu kỳ lặp", "0");
    await user.click(screen.getByRole("button", { name: "Tạo chuỗi" }));
    expect(screen.getByText("Chu kỳ lặp phải từ 1 trở lên")).toBeInTheDocument();
    await replace(user, "Chu kỳ lặp", "2");
    await user.click(screen.getByRole("button", { name: "Tạo chuỗi" }));
    expect(screen.queryByText("Chu kỳ lặp phải từ 1 trở lên")).not.toBeInTheDocument();
    expect(onSubmit).toHaveBeenCalledOnce();
  });
});

describe("RecurringDialog payload", () => {
  it("builds a daily pattern with a parsed duration and fallback name", async () => {
    const { user, onSubmit } = renderDialog();
    await replace(user, "Tên chuỗi lặp", "   ");
    await replace(user, "Thời lượng mỗi lần", "2h");
    await user.click(screen.getByLabelText("Ngày"));
    expect(screen.getByText("ngày làm việc")).toBeInTheDocument();
    await replace(user, "Chu kỳ lặp", "3");
    await user.click(screen.getByRole("button", { name: "Tạo chuỗi" }));
    expect(lastSpec(onSubmit)).toMatchObject({
      name: "Công việc lặp",
      duration: 120,
      pattern: { kind: "daily", every: 3 },
    });
  });

  it("sorts chosen weekdays", async () => {
    const { user, onSubmit } = renderDialog();
    await user.click(screen.getByLabelText("T6"));
    await user.click(screen.getByLabelText("T2"));
    await user.click(screen.getByRole("button", { name: "Tạo chuỗi" }));
    expect(lastSpec(onSubmit).pattern).toEqual({ kind: "weekly", every: 1, weekdays: [0, 2, 4] });
  });

  it("builds a monthly pattern defaulting to the start day", async () => {
    const { user, onSubmit } = renderDialog();
    await user.click(screen.getByLabelText("Tháng"));
    expect(screen.getByLabelText("Ngày trong tháng")).toHaveValue("7");
    await user.click(screen.getByRole("button", { name: "Tạo chuỗi" }));
    expect(lastSpec(onSubmit).pattern).toEqual({ kind: "monthly", every: 1, day: 7 });
  });

  it("ends on a date when chosen, defaulting to 30 days after the start", async () => {
    const { user, onSubmit } = renderDialog();
    await user.click(screen.getByLabelText("Đến ngày"));
    expect(screen.getByLabelText("Số lần lặp")).toBeDisabled();
    const until = screen.getByLabelText("Ngày kết thúc chuỗi");
    expect(until).toBeEnabled();
    expect(until).toHaveValue("06/02/2026 09:00");
    await user.click(screen.getByRole("button", { name: "Tạo chuỗi" }));
    expect(lastSpec(onSubmit).end).toEqual({ kind: "until", date: START + 30 * DAY_MS });

    await user.click(until);
    await user.clear(until);
    await user.type(until, "28/02/2026 17:00{Enter}");
    await user.click(screen.getByRole("button", { name: "Tạo chuỗi" }));
    expect(lastSpec(onSubmit).end).toEqual({ kind: "until", date: at(2026, 2, 28, 17, 0) });
  });

  it("uses the edited start, calendar and resources", async () => {
    const { user, onSubmit } = renderDialog();
    const start = screen.getByLabelText("Bắt đầu chuỗi lặp");
    await user.click(start);
    await user.clear(start);
    await user.type(start, "12/01/2026 10:00{Enter}");
    await user.selectOptions(screen.getByLabelText("Lịch chuỗi lặp"), "Ca đêm");
    await user.click(screen.getByLabelText("NL 11"));
    await user.click(screen.getByLabelText("An"));
    await replace(user, "Đơn vị của An", "0,5");
    await user.click(screen.getByRole("button", { name: "Tạo chuỗi" }));
    expect(lastSpec(onSubmit)).toMatchObject({
      start: at(2026, 1, 12, 10, 0),
      calendar_id: 2,
      resources: [
        { uid: 10, units: 0.5 },
        { uid: 11, units: 1 },
      ],
    });
  });

  it("rejects resource units that are not above zero", async () => {
    const { user, onSubmit } = renderDialog();
    await user.click(screen.getByLabelText("An"));
    await replace(user, "Đơn vị của An", "0");
    await user.click(screen.getByRole("button", { name: "Tạo chuỗi" }));
    expect(screen.getByText("Đơn vị nguồn lực phải lớn hơn 0 (1 = 100%)")).toBeInTheDocument();
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("builds calendar-day, nth-weekday and yearly patterns", async () => {
    const { user, onSubmit } = renderDialog();
    await user.click(screen.getByLabelText("Ngày"));
    await user.click(screen.getByLabelText(/^Ngày lịch/));
    expect(screen.getByText("ngày lịch")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Tạo chuỗi" }));
    expect(lastSpec(onSubmit).pattern).toEqual({ kind: "calendar_daily", every: 1 });

    // 07/01/2026 is the first Wednesday of January.
    await user.click(screen.getByLabelText("Tháng"));
    await user.click(screen.getByLabelText("Theo thứ"));
    expect(screen.getByLabelText("Ngày trong tháng")).toBeDisabled();
    await user.click(screen.getByRole("button", { name: "Tạo chuỗi" }));
    expect(lastSpec(onSubmit).pattern).toEqual({ kind: "monthly_nth", every: 1, ordinal: 1, weekday: 2 });

    await user.selectOptions(screen.getByLabelText("Thứ trong tuần"), "thứ Sáu");
    await user.selectOptions(screen.getByLabelText("Lần thứ mấy trong tháng"), "cuối cùng");
    await user.click(screen.getByLabelText("Năm"));
    await user.selectOptions(screen.getByLabelText("Tháng trong năm"), "Tháng 11");
    await user.click(screen.getByRole("button", { name: "Tạo chuỗi" }));
    expect(lastSpec(onSubmit).pattern).toEqual({ kind: "yearly_nth", every: 1, month: 11, ordinal: 5, weekday: 4 });

    await user.click(screen.getByLabelText("Theo ngày"));
    await replace(user, "Ngày trong tháng", "29");
    await user.click(screen.getByRole("button", { name: "Tạo chuỗi" }));
    expect(lastSpec(onSubmit).pattern).toEqual({ kind: "yearly", every: 1, month: 11, day: 29 });
  });

  it("prefills an existing series and submits it unchanged", async () => {
    const initial: RecurrenceSpec = {
      name: "Báo cáo tháng",
      duration: 240,
      start: at(2026, 1, 30, 13, 0),
      pattern: { kind: "monthly", every: 2, day: 31 },
      end: { kind: "until", date: at(2026, 12, 31) },
      calendar_id: 2,
      resources: [{ uid: 10, units: 0.5 }],
      background: false,
    };
    const { user, onSubmit } = renderDialog({ initial });
    expect(screen.getByLabelText("Đơn vị của An")).toHaveValue("0.5");
    expect(screen.getByRole("heading", { name: "Sửa công việc lặp" })).toBeInTheDocument();
    expect(screen.getByLabelText("Thời lượng mỗi lần")).toHaveValue("0,5 ngày");
    expect(screen.getByLabelText("Ngày trong tháng")).toHaveValue("31");
    expect(screen.getByLabelText("An")).toBeChecked();
    expect(screen.getByText(/Các lần đã sửa riêng được giữ nguyên/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Cập nhật chuỗi" }));
    expect(onSubmit).toHaveBeenCalledExactlyOnceWith(initial);
  });

  it("reads a series saved before units as units 1", async () => {
    const initial: RecurrenceSpec = {
      name: "Họp",
      duration: 60,
      start: START,
      pattern: { kind: "daily", every: 1 },
      end: { kind: "count", count: 2 },
      calendar_id: null,
      resource_uids: [10],
      resources: [],
    };
    const { user, onSubmit } = renderDialog({ initial });
    expect(screen.getByLabelText("An")).toBeChecked();
    expect(screen.getByLabelText("Đơn vị của An")).toHaveValue("1");
    expect(screen.getByLabelText(/Chuỗi nền/)).not.toBeChecked();
    await user.click(screen.getByRole("button", { name: "Cập nhật chuỗi" }));
    expect(lastSpec(onSubmit).resources).toEqual([{ uid: 10, units: 1 }]);
    expect(lastSpec(onSubmit)).not.toHaveProperty("resource_uids");
  });

  it("marks a series as background and keeps the choice when editing", async () => {
    const { user, onSubmit } = renderDialog();
    const box = screen.getByLabelText(/Chuỗi nền/);
    expect(box).not.toBeChecked();
    await user.click(box);
    await user.click(screen.getByRole("button", { name: "Tạo chuỗi" }));
    expect(lastSpec(onSubmit).background).toBe(true);

    cleanup();
    const edited = renderDialog({ initial: { ...lastSpec(onSubmit) } });
    expect(screen.getByLabelText(/Chuỗi nền/)).toBeChecked();
    await edited.user.click(screen.getByRole("button", { name: "Cập nhật chuỗi" }));
    expect(lastSpec(edited.onSubmit).background).toBe(true);
  });

  it("shows a note when there are no resources", () => {
    render(
      <RecurringDialog
        project={makeProject()}
        initial={null}
        defaultStart={START}
        onClose={vi.fn()}
        onSubmit={vi.fn(async () => true)}
      />,
    );
    expect(screen.getByText("Chưa có nguồn lực")).toBeInTheDocument();
  });
});

describe("recurringCommand", () => {
  const spec: RecurrenceSpec = {
    name: "Họp",
    duration: 60,
    start: START,
    pattern: { kind: "daily", every: 1 },
    end: { kind: "count", count: 2 },
    calendar_id: null,
    resources: [],
  };

  it("creates a new series after the selected task", () => {
    expect(recurringCommand([makeTask({ uid: 1 })], 0, 1, spec)).toEqual({
      type: "create_recurring_task",
      after_uid: 1,
      spec,
    });
  });

  it("updates the series when editing a recurring summary", () => {
    const series = makeTask({ uid: 5, summary: true, recurrence: { spec, skipped: [] } });
    expect(recurringCommand([series], 5, 1, spec)).toEqual({ type: "update_recurring_task", uid: 5, spec });
  });

  it("falls back to create when the edited task is no longer a series", () => {
    expect(recurringCommand([makeTask({ uid: 5 })], 5, null, spec)).toMatchObject({
      type: "create_recurring_task",
      after_uid: null,
    });
  });

  it("round-trips through the fake backend like App does", async () => {
    fakeBackend.reset(makeSnapshot(project()));

    function Harness() {
      const [snap, setSnap] = useState(fakeBackend.snapshot);
      const [open, setOpen] = useState(true);
      return open ? (
        <RecurringDialog
          project={snap.project}
          initial={null}
          defaultStart={START}
          onClose={() => setOpen(false)}
          onSubmit={async (next) => {
            const result = await applyCommand(recurringCommand(snap.project.tasks, 0, null, next));
            setSnap(result);
            return result;
          }}
        />
      ) : (
        <p>{snap.project.tasks.map((task) => task.name).join(", ")}</p>
      );
    }

    const user = userEvent.setup();
    render(<Harness />);
    await replace(user, "Tên chuỗi lặp", "Họp giao ban");
    await user.click(screen.getByRole("button", { name: "Tạo chuỗi" }));

    expect(fakeBackend.commands()).toEqual([
      {
        type: "create_recurring_task",
        after_uid: null,
        spec: expect.objectContaining({ name: "Họp giao ban", pattern: { kind: "weekly", every: 1, weekdays: [2] } }),
      },
    ]);
    expect(await screen.findByText("Họp giao ban")).toBeInTheDocument();
  });

  it("keeps the dialog open when the backend errors", async () => {
    fakeBackend.failNext("apply_command", "Chuỗi lặp không hợp lệ");
    const onClose = vi.fn();
    const user = userEvent.setup();
    render(
      <RecurringDialog
        project={project()}
        initial={null}
        defaultStart={START}
        onClose={onClose}
        onSubmit={(next) => applyCommand(recurringCommand([], 0, null, next)).catch(() => undefined)}
      />,
    );
    await user.click(screen.getByRole("button", { name: "Tạo chuỗi" }));
    expect(fakeBackend.calls).toHaveLength(1);
    expect(onClose).not.toHaveBeenCalled();
  });
});

describe("RecurringDialog in English", () => {
  afterEach(() => setCurrentLang("vi"));

  function renderEnglish(onSubmit = vi.fn(async () => true)) {
    render(
      <I18nProvider lang="en">
        <RecurringDialog project={project()} initial={null} defaultStart={START} onClose={() => {}} onSubmit={onSubmit} />
      </I18nProvider>,
    );
    return { user: userEvent.setup(), onSubmit };
  }

  it("shows English labels, weekdays, plurals and defaults", async () => {
    const { user, onSubmit } = renderEnglish();
    expect(screen.getByRole("heading", { name: "Recurring Task" })).toBeInTheDocument();
    expect(screen.getByLabelText("Series name")).toHaveValue("Recurring meeting");
    expect(screen.getByLabelText("Duration of each occurrence")).toHaveValue("1 hr");
    expect(screen.getByLabelText("Weekly")).toBeChecked();
    expect(screen.getByLabelText("Wed")).toBeChecked();
    expect(screen.getByText("week")).toBeInTheDocument();
    expect(screen.getByLabelText("End after")).toBeChecked();
    expect(screen.getByText("occurrences")).toBeInTheDocument();
    expect(screen.getByLabelText("Series start")).toHaveValue("2026-01-07 09:00");
    expect(screen.getByRole("option", { name: "Project calendar" })).toBeInTheDocument();
    expect(screen.getByText("Background series: does not set the project finish or the critical path")).toBeInTheDocument();
    expect(screen.getByLabelText("Resource 11")).toBeInTheDocument();

    await user.clear(screen.getByLabelText("Recurrence interval"));
    await user.type(screen.getByLabelText("Recurrence interval"), "2");
    expect(screen.getByText("weeks")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Create series" }));
    expect(lastSpec(onSubmit).name).toBe("Recurring meeting");
  });

  it("puts the ordinal before the weekday and names months", async () => {
    const { user } = renderEnglish();
    await user.click(screen.getByLabelText("Yearly"));
    expect(screen.getByRole("combobox", { name: "Month of the year" })).toHaveDisplayValue("January");
    await user.click(screen.getByLabelText("The"));
    const ordinal = screen.getByRole("combobox", { name: "Which one in the month" });
    const weekday = screen.getByRole("combobox", { name: "Day of the week" });
    expect(ordinal).toHaveDisplayValue("first");
    expect(weekday).toHaveDisplayValue("Wednesday");
    expect(ordinal.compareDocumentPosition(weekday) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(screen.getByText("of the month")).toBeInTheDocument();
  });

  it("explains problems in English", async () => {
    const { user, onSubmit } = renderEnglish();
    await user.click(screen.getByLabelText("Wed"));
    await user.click(screen.getByRole("button", { name: "Create series" }));
    expect(screen.getByText("Select at least one day of the week")).toBeInTheDocument();
    expect(onSubmit).not.toHaveBeenCalled();
  });
});
