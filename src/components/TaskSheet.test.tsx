import { fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createRef } from "react";
import { afterEach, describe, expect, it, vi, type Mock } from "vitest";

import { I18nProvider, setCurrentLang, type Lang } from "../i18n";
import { at, makeAssignment, makeResource, makeTask, outlineProject, standardCalendar } from "../test/fixtures";
import type { Task } from "../types";
import { buildViewRows, type ViewRow } from "../viewRows";
import { TaskSheet } from "./TaskSheet";

const HANDLERS = [
  "onSelect",
  "onFillField",
  "onRename",
  "onDuration",
  "onStart",
  "onPredecessors",
  "onPercent",
  "onRemaining",
  "onActualStart",
  "onActualFinish",
  "onRemainingWork",
  "onOvertime",
  "onCalendar",
  "onEditSeries",
  "onScroll",
] as const;

function renderSheet(rows: ViewRow[], options: { tracking?: boolean; selected?: number | null; lang?: Lang } = {}) {
  const handlers = Object.fromEntries(HANDLERS.map((name) => [name, vi.fn<(...args: unknown[]) => void>()])) as Record<
    (typeof HANDLERS)[number],
    Mock<(...args: unknown[]) => void>
  >;
  const bodyRef = createRef<HTMLDivElement>();
  const utils = render(
    <TaskSheet
      rows={rows}
      calendars={[standardCalendar(1), standardCalendar(2, "Ca đêm")]}
      assignments={[
        makeAssignment({ uid: 1, task_uid: 2, resource_uid: 10, overtime_work: 60 }),
        makeAssignment({ uid: 2, task_uid: 2, resource_uid: 11, overtime_work: 30, units: 0.5 }),
      ]}
      resources={[makeResource({ uid: 10, name: "An" }), makeResource({ uid: 11, name: "Bình" })]}
      selected={options.selected ?? null}
      tracking={options.tracking ?? false}
      bodyRef={bodyRef}
      {...handlers}
    />,
    { wrapper: ({ children }) => <I18nProvider lang={options.lang ?? "vi"}>{children}</I18nProvider> },
  );
  const head = utils.container.querySelector<HTMLElement>(".sheet-head");
  if (!head) throw new Error("missing header");
  return { ...utils, head, handlers, user: userEvent.setup() };
}

function taskRows(container: HTMLElement): HTMLElement[] {
  return [...container.querySelectorAll<HTMLElement>(".sheet-row:not(.group-head)")];
}

function rowOf(container: HTMLElement, uid: number): HTMLElement {
  const rows = taskRows(container);
  const index = outlineProject().tasks.findIndex((task) => task.uid === uid);
  return rows[index];
}

function flatRows(tasks: Task[] = outlineProject().tasks): ViewRow[] {
  return buildViewRows({ ...outlineProject(), tasks }, "none", null, "none", "none");
}

const BASE_LABELS = ["ID", "WBS", "Tên", "Thời lượng", "Bắt đầu", "Kết thúc", "Tiền nhiệm", "Nguồn lực", "Trễ"];
const TRACKING_LABELS = [
  "ID", "WBS", "Tên", "Thời lượng", "%", "Còn lại", "Bắt đầu", "Kết thúc",
  "Bắt đầu thực", "Kết thúc thực", "Việc còn lại", "Làm thêm", "Lịch", "Tiền nhiệm", "Nguồn lực", "Trễ",
];

describe("TaskSheet columns", () => {
  it.each([
    [false, BASE_LABELS],
    [true, TRACKING_LABELS],
  ])("tracking=%s renders the header labels", (tracking, labels) => {
    const { head } = renderSheet(flatRows(), { tracking });
    expect([...head.children].map((child) => child.textContent)).toEqual(labels);
  });

  it("shows the task's resource names the way Project writes them", () => {
    const { container } = renderSheet(flatRows());
    expect(within(rowOf(container, 2)).getByText("An, Bình[50%]")).toBeInTheDocument();
  });

  it.each([false, true])("tracking=%s: every task row has one cell per header column and the same tracks", (tracking) => {
    const tasks = outlineProject().tasks;
    tasks[1] = { ...tasks[1], recurrence: { spec: {} as never, skipped: [] } };
    tasks[2] = { ...tasks[2], series_uid: 2, detached: true };
    const { container, head } = renderSheet(flatRows(tasks), { tracking });
    const rows = taskRows(container);
    expect(rows).toHaveLength(8);
    for (const row of rows) {
      expect(row.children).toHaveLength(head.children.length);
      expect(row.style.gridTemplateColumns).toBe(head.style.gridTemplateColumns);
      expect(row.style.width).toBe(head.style.width);
    }
  });

  it("sizes the header from the column widths plus gaps and padding", () => {
    const { head } = renderSheet(flatRows());
    expect(head.style.gridTemplateColumns).toBe("42px 72px 200px 100px 168px 168px 120px 160px 80px");
    // 1110px of tracks + 8 gaps × 4px + 12px padding.
    expect(head.style.width).toBe("1154px");
    expect(head.style.minWidth).toBe("1154px");
  });

  describe("with a wide pane", () => {
    afterEach(() => {
      vi.restoreAllMocks();
    });

    it("gives the extra width to the name column in header and body alike", () => {
      vi.spyOn(Element.prototype, "clientWidth", "get").mockImplementation(function (this: Element) {
        return this.classList.contains("sheet-body") ? 1500 : 0;
      });
      const { container, head } = renderSheet(flatRows());
      expect(head.style.gridTemplateColumns).toBe("42px 72px 546px 100px 168px 168px 120px 160px 80px");
      expect(head.style.width).toBe("1500px");
      const canvas = container.querySelector<HTMLElement>(".sheet-body > div");
      expect(canvas?.style.width).toBe("1500px");
      for (const row of taskRows(container)) {
        expect(row.style.gridTemplateColumns).toBe(head.style.gridTemplateColumns);
      }
    });
  });

  it("keeps the header scrolled with the body", () => {
    const { container } = renderSheet(flatRows());
    const body = container.querySelector<HTMLElement>(".sheet-body");
    const clip = container.querySelector<HTMLElement>(".sheet-head-clip");
    if (!body || !clip) throw new Error("missing sheet parts");
    body.scrollLeft = 120;
    fireEvent.scroll(body);
    expect(clip.scrollLeft).toBe(120);
  });
});

describe("TaskSheet rows", () => {
  it("renders task fields", () => {
    const tasks = outlineProject().tasks;
    tasks[1] = {
      ...tasks[1],
      duration: 960,
      start: at(2026, 1, 5, 8),
      finish: at(2026, 1, 6, 17),
      predecessors: "1FS+2d",
      total_slack: -480,
    };
    const { container } = renderSheet(flatRows(tasks));
    const row = rowOf(container, 2);
    const cells = [...row.children] as HTMLElement[];
    expect(cells[0]).toHaveTextContent("2");
    expect(cells[1]).toHaveTextContent("1.1");
    expect(within(row).getByDisplayValue("Phác thảo")).toBeInTheDocument();
    expect(within(row).getByDisplayValue("2 ngày")).toBeInTheDocument();
    expect(within(row).getByLabelText("Bắt đầu")).toHaveValue("05/01/2026 08:00");
    expect(cells[5]).toHaveTextContent("06/01/2026 17:00");
    expect(within(row).getByDisplayValue("1FS+2d")).toBeInTheDocument();
    expect(cells[7]).toHaveTextContent("An, Bình[50%]");
    expect(cells[8]).toHaveTextContent("-1 ngày");
    expect(cells[8]).toHaveClass("bad");
    expect(rowOf(container, 3).children[8]).not.toHaveClass("bad");
  });

  it("indents names by outline level", () => {
    const { container } = renderSheet(flatRows());
    const name = (uid: number) => rowOf(container, uid).querySelector<HTMLInputElement>("input.name");
    expect(name(1)?.style.paddingLeft).toBe("8px");
    expect(name(2)?.style.paddingLeft).toBe("22px");
    expect(name(5)?.style.paddingLeft).toBe("36px");
  });

  it("marks selected, critical and highlighted rows", () => {
    const rows = buildViewRows(outlineProject(), "none", null, "none", "late");
    const { container } = renderSheet(rows, { selected: 5 });
    expect(rowOf(container, 2)).toHaveClass("critical");
    expect(rowOf(container, 3)).toHaveClass("highlighted");
    expect(rowOf(container, 5)).toHaveClass("selected");
    expect(rowOf(container, 7).className).toBe("sheet-row");
  });

  it("disables duration and start on summaries", () => {
    const { container } = renderSheet(flatRows());
    const summary = rowOf(container, 1);
    expect(within(summary).getByDisplayValue("1 ngày")).toBeDisabled();
    expect(within(summary).getByLabelText("Bắt đầu")).toBeDisabled();
    expect(within(rowOf(container, 2)).getByLabelText("Bắt đầu")).toBeEnabled();
  });

  it("renders group headers between task rows", () => {
    const rows = buildViewRows(outlineProject(), "none", null, "summary", "none");
    const { container } = renderSheet(rows);
    const heads = [...container.querySelectorAll(".sheet-row.group-head")];
    expect(heads.map((head) => head.textContent)).toEqual([
      "1 Thiết kế", "1.3 Kiểm thử", "2 Thi công", "Không có summary",
    ]);
    const tops = [...container.querySelectorAll<HTMLElement>(".sheet-row")].map((row) => row.style.top);
    expect(tops).toEqual(rows.map((_, index) => `${index * 28}px`));
  });

  it("shows series markers and opens the series editor without selecting the row", async () => {
    const tasks = outlineProject().tasks;
    tasks[0] = { ...tasks[0], recurrence: { spec: {} as never, skipped: [] } };
    tasks[1] = { ...tasks[1], series_uid: 1 };
    tasks[2] = { ...tasks[2], series_uid: 1, detached: true };
    const { container, handlers, user } = renderSheet(flatRows(tasks));
    expect(rowOf(container, 2).querySelector(".series-icon.occurrence")).toHaveTextContent("·");
    expect(rowOf(container, 3).querySelector(".series-icon.detached")).toHaveTextContent("↺");
    await user.click(screen.getByRole("button", { name: "Sửa chuỗi lặp" }));
    expect(handlers.onEditSeries).toHaveBeenCalledExactlyOnceWith(1);
    expect(handlers.onSelect).not.toHaveBeenCalled();
  });

  it("only renders the visible window of a long list", () => {
    const tasks = Array.from({ length: 200 }, (_, index) => makeTask({ uid: index + 1 }));
    const { container } = renderSheet(flatRows(tasks));
    expect(taskRows(container).length).toBeLessThan(200);
    expect(container.querySelector<HTMLElement>(".sheet-body > div")?.style.height).toBe(`${200 * 28}px`);
  });
});

describe("TaskSheet editing", () => {
  it("selects a row on click", async () => {
    const { container, handlers, user } = renderSheet(flatRows());
    await user.click(rowOf(container, 7).children[0]);
    expect(handlers.onSelect).toHaveBeenCalledWith(7, false);
  });

  it("extends the selection with Shift+click", async () => {
    const { container, handlers, user } = renderSheet(flatRows());
    await user.keyboard("{Shift>}");
    await user.click(rowOf(container, 7).children[0]);
    await user.keyboard("{/Shift}");
    expect(handlers.onSelect).toHaveBeenCalledWith(7, true);
  });

  it("tells which column fill-down copies when a cell gets focus", async () => {
    const { container, handlers, user } = renderSheet(flatRows());
    await user.click(within(rowOf(container, 2)).getByDisplayValue("Phác thảo"));
    expect(handlers.onFillField).toHaveBeenLastCalledWith("name");
    const cells = rowOf(container, 2).querySelectorAll("input");
    await user.click(cells[1]);
    expect(handlers.onFillField).toHaveBeenLastCalledWith("duration");
    // The ID column is read-only text; the WBS cell holds no field either.
    rowOf(container, 2).children[1].dispatchEvent(new FocusEvent("focusin", { bubbles: true }));
    expect(handlers.onFillField).toHaveBeenLastCalledWith(null);
  });

  it("renames on blur with trimmed text and ignores empty names", async () => {
    const { container, handlers, user } = renderSheet(flatRows());
    const name = within(rowOf(container, 2)).getByDisplayValue("Phác thảo");
    await user.clear(name);
    await user.type(name, "  Bản vẽ  ");
    await user.tab();
    expect(handlers.onRename).toHaveBeenCalledExactlyOnceWith(2, "Bản vẽ");

    const other = within(rowOf(container, 3)).getByDisplayValue("Duyệt");
    await user.clear(other);
    await user.tab();
    expect(handlers.onRename).toHaveBeenCalledOnce();
    expect(other).toHaveValue("Duyệt");
  });

  it("parses durations and skips unchanged or invalid input", async () => {
    const { container, handlers, user } = renderSheet(flatRows());
    const duration = within(rowOf(container, 2)).getByDisplayValue("1 ngày");
    await user.clear(duration);
    await user.type(duration, "3h");
    await user.tab();
    expect(handlers.onDuration).toHaveBeenCalledExactlyOnceWith(2, 180);

    const second = within(rowOf(container, 3)).getByDisplayValue("1 ngày");
    await user.click(second);
    await user.tab();
    await user.clear(second);
    await user.type(second, "lâu");
    await user.tab();
    expect(handlers.onDuration).toHaveBeenCalledOnce();
  });

  it("commits predecessors text", async () => {
    const { container, handlers, user } = renderSheet(flatRows());
    const cells = rowOf(container, 3).querySelectorAll("input");
    const predecessors = cells[cells.length - 1];
    await user.type(predecessors, "2SS");
    await user.tab();
    expect(handlers.onPredecessors).toHaveBeenCalledExactlyOnceWith(3, "2SS");
  });

  it("tracking cells commit percent, overtime and calendar", async () => {
    const { container, handlers, user } = renderSheet(flatRows(), { tracking: true });
    const row = rowOf(container, 2);
    const percent = within(row).getByDisplayValue("0");
    await user.clear(percent);
    await user.type(percent, "37,5");
    await user.tab();
    expect(handlers.onPercent).toHaveBeenCalledExactlyOnceWith(2, 37.5);

    // Overtime sums every assignment on the task: 60 + 30 minutes.
    expect(within(row).getByDisplayValue("1,5 giờ")).toBeInTheDocument();

    await user.selectOptions(within(row).getByRole("combobox"), "Ca đêm");
    expect(handlers.onCalendar).toHaveBeenCalledExactlyOnceWith(2, 2);
    await user.selectOptions(within(row).getByRole("combobox"), "Lịch dự án");
    expect(handlers.onCalendar).toHaveBeenLastCalledWith(2, null);
  });

  it("tabbing through or clearing the percent cell does not change the task", async () => {
    const tasks = outlineProject().tasks;
    tasks[1] = { ...tasks[1], percent_complete: 37.5 };
    const { container, handlers, user } = renderSheet(flatRows(tasks), { tracking: true });
    const percent = within(rowOf(container, 2)).getByDisplayValue("38");
    await user.click(percent);
    await user.tab();
    await user.clear(percent);
    await user.tab();
    await user.clear(percent);
    await user.type(percent, "abc");
    await user.tab();
    expect(handlers.onPercent).not.toHaveBeenCalled();
    expect(percent).toHaveValue("38");
  });

  it("tracking actual dates are optional and clearable", async () => {
    const tasks = outlineProject().tasks;
    tasks[1] = { ...tasks[1], actual_start: at(2026, 1, 5, 8) };
    const { container, handlers, user } = renderSheet(flatRows(tasks), { tracking: true });
    const row = rowOf(container, 2);
    const [actualStart, actualFinish] = within(row).getAllByLabelText("Ngày thực");
    expect(actualStart).toHaveValue("05/01/2026 08:00");
    expect(actualFinish).toHaveValue("");
    await user.click(within(row).getByRole("button", { name: "Xóa ngày" }));
    expect(handlers.onActualStart).toHaveBeenCalledExactlyOnceWith(2, null);
  });
});

describe("TaskSheet in English", () => {
  afterEach(() => setCurrentLang("vi"));

  it("labels the tracking columns with Project's English names", () => {
    const { head } = renderSheet(flatRows(), { tracking: true, lang: "en" });
    expect([...head.children].map((child) => child.textContent)).toEqual([
      "ID", "WBS", "Name", "Duration", "%", "Remaining", "Start", "Finish",
      "Actual start", "Actual finish", "Remaining work", "Overtime", "Calendar", "Predecessors", "Resource names", "Slack",
    ]);
    expect(screen.getAllByRole("option", { name: "Project calendar" }).length).toBeGreaterThan(0);
    expect(screen.getAllByLabelText("Actual date").length).toBeGreaterThan(0);
  });

  it("names the series marker in English", () => {
    const tasks = outlineProject().tasks;
    tasks[0] = { ...tasks[0], recurrence: { spec: {} as never, skipped: [] } };
    tasks[1] = { ...tasks[1], series_uid: 1 };
    const { container } = renderSheet(flatRows(tasks), { lang: "en" });
    expect(screen.getByRole("button", { name: "Edit recurring series" })).toHaveAttribute(
      "title",
      "Recurring task — click to edit the series",
    );
    expect(rowOf(container, 2).querySelector(".series-icon.occurrence")).toHaveAttribute(
      "title",
      "One occurrence of a recurring series",
    );
  });
});
