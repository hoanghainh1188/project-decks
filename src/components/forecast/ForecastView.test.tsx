import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { I18nProvider } from "../../i18n";
import { makeProject, makeTask } from "../../test/fixtures";
import type { Forecast } from "../../types";
import { ForecastView } from "./ForecastView";
import type { ForecastState } from "./useForecast";

const DAY = 86_400_000;
const MON = Date.UTC(2026, 0, 5, 17);

const project = makeProject({
  tasks: [
    makeTask({ uid: 1, name: "Thiết kế", work_low: 240, work_high: 960, deadline: MON + DAY }),
    makeTask({ uid: 2, name: "Lập trình" }),
  ],
});

const result: Forecast = {
  iterations: 1000,
  planned_finish: MON,
  leveled_finish: MON + DAY,
  finish: { p50: MON + 2 * DAY, p80: MON + 3 * DAY, p90: MON + 4 * DAY },
  histogram: [
    [Date.UTC(2026, 0, 7), 500],
    [Date.UTC(2026, 0, 8), 300],
    [Date.UTC(2026, 0, 9), 200],
  ],
  tasks: [
    { uid: 1, p50: MON + DAY, p80: MON + 2 * DAY, p90: MON + 3 * DAY, on_time: 0.42, criticality: 0.9 },
    { uid: 2, p50: MON + 2 * DAY, p80: MON + 3 * DAY, p90: MON + 4 * DAY, on_time: null, criticality: 0.7 },
  ],
};

function renderView(state: Partial<ForecastState>, lang: "vi" | "en" = "vi", data = project) {
  const onPickTask = vi.fn();
  render(
    <I18nProvider lang={lang}>
      <ForecastView project={data} state={{ result: null, current: false, running: false, error: null, ...state }} onPickTask={onPickTask} />
    </I18nProvider>,
  );
  return { onPickTask };
}

describe("ForecastView", () => {
  it("says it is working before the first result", () => {
    renderView({ running: true });
    expect(screen.getByRole("status")).toHaveTextContent("Đang tính dự báo");
  });

  it("shows the finish dates against the plan", () => {
    renderView({ result, current: true });
    const summary = screen.getByRole("list", { name: "Ngày xong dự án" });
    expect(within(summary).getByText("Theo lịch hiện tại").parentElement).toHaveTextContent("05/01/2026");
    expect(within(summary).getByText("Khi đã cân bằng").parentElement).toHaveTextContent("06/01/2026");
    expect(within(summary).getByText("90% khả năng xong trước").parentElement).toHaveTextContent("09/01/2026");
    expect(within(summary).getByText("90% khả năng xong trước").parentElement).toHaveTextContent("+3 ngày");
    expect(screen.getByRole("img", { name: /Phân bố ngày xong dự án qua 1000 lần mô phỏng/ })).toBeInTheDocument();
  });

  it("lists the risky tasks and opens one on the Gantt", () => {
    const { onPickTask } = renderView({ result, current: true });
    const table = screen.getByRole("table", { name: "Việc cần chú ý" });
    const rows = within(table).getAllByRole("row").slice(1);
    expect(rows[0]).toHaveTextContent("Thiết kế");
    expect(rows[0]).toHaveTextContent("42%");
    fireEvent.click(within(rows[1]).getByRole("button", { name: "Lập trình" }));
    expect(onPickTask).toHaveBeenCalledWith(2);
  });

  it("notes a result for an older project and a project with no ranges", () => {
    const plain = makeProject({ tasks: [makeTask({ uid: 2, name: "Lập trình" })] });
    renderView({ result, current: false, running: true }, "vi", plain);
    expect(screen.getByRole("status")).toHaveTextContent("Đang tính lại");
    expect(screen.getByText(/Chưa việc nào có khoảng effort/)).toBeInTheDocument();
  });

  it("shows a failure", () => {
    renderView({ error: "Không có thời gian làm việc" });
    expect(screen.getByRole("alert")).toHaveTextContent("Không có thời gian làm việc");
  });

  it("speaks English", () => {
    renderView({ result, current: true }, "en");
    expect(screen.getByRole("list", { name: "Project finish" })).toBeInTheDocument();
    expect(screen.getByRole("table", { name: "Tasks to watch" })).toBeInTheDocument();
  });
});
