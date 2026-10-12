import { fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import { I18nProvider, setCurrentLang } from "../i18n";
import { at, makeEvm, makeMetrics } from "../test/fixtures";
import type { ProjectMetrics } from "../types";
import { EvmPanel, formatCompactMoney, formatTickDate, formatWorkHours } from "./EvmPanel";

function renderPanel(metrics: ProjectMetrics | null) {
  const handlers = { onOpenProjectInfo: vi.fn(), onOpenBaseline: vi.fn(), onClose: vi.fn() };
  render(<EvmPanel metrics={metrics} {...handlers} />);
  return { user: userEvent.setup(), ...handlers };
}

function cell(label: string): HTMLElement {
  const row = screen.getByRole("rowheader", { name: label }).closest("tr");
  if (!row) throw new Error(`No row for ${label}`);
  return within(row).getByRole("cell");
}

describe("EvmPanel states", () => {
  it("shows a loading note before metrics arrive", () => {
    renderPanel(null);
    expect(screen.getByText("Đang tải số liệu…")).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });

  it("explains a missing baseline and links to the baseline dialog", async () => {
    const { user, onOpenBaseline } = renderPanel(makeMetrics({ has_baseline: false, metrics: null }));
    expect(screen.getByText(/Chưa có đường cơ sở 0/)).toBeInTheDocument();
    expect(screen.queryByText(/Chưa có ngày trạng thái/)).not.toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Đặt đường cơ sở…" }));
    expect(onOpenBaseline).toHaveBeenCalledOnce();
  });

  it("explains a missing status date and links to project info", async () => {
    const { user, onOpenProjectInfo } = renderPanel(makeMetrics({ status_date: null, metrics: null }));
    expect(screen.getByText(/Chưa có ngày trạng thái/)).toBeInTheDocument();
    expect(screen.queryByText(/Tính đến ngày trạng thái/)).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Đặt ngày trạng thái…" }));
    expect(onOpenProjectInfo).toHaveBeenCalledOnce();
  });

  it("shows both messages when baseline and status date are missing", () => {
    renderPanel(makeMetrics({ status_date: null, has_baseline: false, metrics: null }));
    expect(screen.getByRole("button", { name: "Đặt đường cơ sở…" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Đặt ngày trạng thái…" })).toBeInTheDocument();
  });

  it("closes from the header button", async () => {
    const { user, onClose } = renderPanel(makeMetrics());
    await user.click(screen.getByRole("button", { name: "Đóng bảng EVM" }));
    expect(onClose).toHaveBeenCalledOnce();
  });
});

describe("EvmPanel enlarged charts", () => {
  it("opens burn-down and S-curve in a dialog and returns focus on Escape", async () => {
    const { user } = renderPanel(
      makeMetrics(
        {},
        {
          baseline_work: 4800,
          current_remaining_work: 2400,
          ideal: [
            { date: at(2026, 1, 5), remaining_work: 4800 },
            { date: at(2026, 1, 16), remaining_work: 0 },
          ],
        },
      ),
    );
    const zoom = screen.getByRole("button", { name: "Phóng to" });
    await user.click(zoom);
    const dialog = screen.getByRole("dialog", { name: "Burn-down và đường cong S" });
    expect(dialog).toHaveFocus();
    expect(within(dialog).getByRole("img", { name: /Biểu đồ burn-down/ })).toHaveAttribute("viewBox", "0 0 880 340");
    const close = within(dialog).getByRole("button", { name: "Đóng hộp thoại" });
    await user.tab();
    expect(close).toHaveFocus();
    await user.tab({ shift: true });
    expect(close).toHaveFocus();
    await user.click(within(dialog).getByRole("img", { name: /Biểu đồ burn-down/ }));
    expect(dialog).toHaveFocus();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(zoom).toHaveFocus();
  });

  it("closes from a click on the backdrop", async () => {
    const { user } = renderPanel(makeMetrics());
    await user.click(screen.getByRole("button", { name: "Phóng to" }));
    const backdrop = screen.getByRole("dialog").parentElement as HTMLElement;
    fireEvent.mouseDown(backdrop);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("is off until metrics arrive", () => {
    renderPanel(null);
    expect(screen.getByRole("button", { name: "Phóng to" })).toBeDisabled();
  });
});

describe("EvmPanel metric table", () => {
  it("renders every metric with money and index formatting", () => {
    renderPanel(makeMetrics({ status_date: at(2026, 1, 15) }));
    expect(screen.getByText("Tính đến ngày trạng thái 15/01/2026.")).toBeInTheDocument();
    expect(screen.getAllByRole("row")).toHaveLength(11);
    expect(cell("Giá trị kế hoạch (PV, BCWS)")).toHaveTextContent("4.000.000");
    expect(cell("Giá trị đạt được (EV, BCWP)")).toHaveTextContent("3.000.000");
    expect(cell("Chi phí thực tế (AC, ACWP)")).toHaveTextContent("3.500.000");
    expect(cell("Chênh lệch tiến độ (SV)")).toHaveTextContent("-1.000.000");
    expect(cell("Chỉ số tiến độ (SPI)")).toHaveTextContent("0,75");
    expect(cell("Chỉ số chi phí (CPI)")).toHaveTextContent("0,86");
    expect(cell("Ngân sách khi hoàn thành (BAC)")).toHaveTextContent("10.000.000");
    expect(cell("Ước tính khi hoàn thành (EAC)")).toHaveTextContent("11.666.667");
  });

  it("shows — with a hint for undefined values", () => {
    renderPanel(makeMetrics({ metrics: makeEvm({ spi: null, cpi: null, eac: null, etc: null, vac: null }) }));
    for (const label of [
      "Chỉ số tiến độ (SPI)",
      "Chỉ số chi phí (CPI)",
      "Ước tính khi hoàn thành (EAC)",
      "Ước tính phần còn lại (ETC)",
      "Chênh lệch khi hoàn thành (VAC)",
    ]) {
      expect(cell(label)).toHaveTextContent(/^—$/);
      expect(cell(label)).toHaveAttribute("title", "Không xác định: mẫu số không lớn hơn 0");
      expect(cell(label)).not.toHaveClass("bad");
    }
    expect(cell("Giá trị kế hoạch (PV, BCWS)")).not.toHaveAttribute("title");
  });

  it("marks negative variances and indexes below 1 in red", () => {
    renderPanel(makeMetrics());
    expect(cell("Chênh lệch tiến độ (SV)")).toHaveClass("bad");
    expect(cell("Chênh lệch chi phí (CV)")).toHaveClass("bad");
    expect(cell("Chênh lệch khi hoàn thành (VAC)")).toHaveClass("bad");
    expect(cell("Chỉ số tiến độ (SPI)")).toHaveClass("bad");
    expect(cell("Chỉ số chi phí (CPI)")).toHaveClass("bad");
    expect(cell("Giá trị kế hoạch (PV, BCWS)")).not.toHaveClass("bad");
  });

  it("keeps on-plan and ahead values neutral", () => {
    renderPanel(makeMetrics({ metrics: makeEvm({ sv: 0, cv: 250_000, vac: 0, spi: 1, cpi: 1.2 }) }));
    for (const label of [
      "Chênh lệch tiến độ (SV)",
      "Chênh lệch chi phí (CV)",
      "Chênh lệch khi hoàn thành (VAC)",
      "Chỉ số tiến độ (SPI)",
      "Chỉ số chi phí (CPI)",
    ]) {
      expect(cell(label)).not.toHaveClass("bad");
    }
    expect(cell("Chỉ số chi phí (CPI)")).toHaveTextContent("1,20");
  });

  it("never marks money totals red, even when negative", () => {
    renderPanel(makeMetrics({ metrics: makeEvm({ ac: -10, etc: -5 }) }));
    expect(cell("Chi phí thực tế (AC, ACWP)")).not.toHaveClass("bad");
    expect(cell("Ước tính phần còn lại (ETC)")).not.toHaveClass("bad");
  });
});

describe("EvmPanel burn-down", () => {
  it("explains when there is no data", () => {
    renderPanel(makeMetrics({}, { current_remaining_work: 960 }));
    expect(screen.getByText("Việc còn lại hiện tại: 16 giờ")).toBeInTheDocument();
    expect(screen.getByText(/Chưa có dữ liệu/)).toBeInTheDocument();
    expect(screen.queryByRole("img")).not.toBeInTheDocument();
  });

  it("draws the ideal line and asks for a first actual point", () => {
    renderPanel(
      makeMetrics(
        {},
        {
          baseline_work: 4800,
          current_remaining_work: 2400,
          ideal: [
            { date: at(2026, 1, 5), remaining_work: 4800 },
            { date: at(2026, 1, 16), remaining_work: 0 },
          ],
        },
      ),
    );
    expect(screen.getByRole("img", { name: /Biểu đồ burn-down/ })).toBeInTheDocument();
    expect(screen.getByText(/Công việc baseline: 80 giờ/)).toBeInTheDocument();
    expect(screen.getByText(/Chưa có điểm thực tế/)).toBeInTheDocument();
  });

  it("draws actual points with a tooltip and the status-date legend", () => {
    const { container } = render(
      <EvmPanel
        metrics={makeMetrics(
          {},
          {
            status_date: at(2026, 1, 9),
            actual: [{ date: at(2026, 1, 9), remaining_work: 1500, ev: 0, ac: 0 }],
          },
        )}
        onOpenProjectInfo={vi.fn()}
        onOpenBaseline={vi.fn()}
        onClose={vi.fn()}
      />,
    );
    expect(container.querySelectorAll("circle")).toHaveLength(1);
    expect(container.querySelector("circle title")).toHaveTextContent("09/01/2026: còn 25 giờ");
    expect(screen.getByText("Ngày trạng thái")).toBeInTheDocument();
    expect(screen.queryByText(/Chưa có điểm thực tế/)).not.toBeInTheDocument();
  });
});

describe("EvmPanel S-curve", () => {
  it("explains that a baseline is needed when there are no points", () => {
    renderPanel(makeMetrics());
    expect(screen.getByText("Cần đường cơ sở 0 để vẽ đường cong S.")).toBeInTheDocument();
    expect(screen.queryByRole("img", { name: /Đường cong S/ })).not.toBeInTheDocument();
  });

  it("draws PV through the baseline finish and EV/AC up to the status date", () => {
    const { container } = render(
      <EvmPanel
        metrics={makeMetrics(
          {},
          {},
          {
            status_date: at(2026, 1, 6, 12),
            baseline_finish: at(2026, 1, 7),
            step: "week",
            points: [
              { date: at(2026, 1, 5), pv: 0, ev: 0, ac: 0 },
              { date: at(2026, 1, 6, 12), pv: 1_750, ev: 750, ac: 1_000 },
              { date: at(2026, 1, 7), pv: 3_000, ev: null, ac: null },
            ],
          },
        )}
        onOpenProjectInfo={vi.fn()}
        onOpenBaseline={vi.fn()}
        onClose={vi.fn()}
      />,
    );
    const chart = screen.getByRole("img", { name: /Đường cong S/ });
    expect(screen.getByText(/cộng dồn theo tuần/)).toBeInTheDocument();
    const paths = Array.from(chart.querySelectorAll("path")).map((path) => path.getAttribute("d") ?? "");
    // PV has three vertices; EV and AC stop at the status date with two each.
    expect(paths.map((d) => d.split(/[ML]/).filter(Boolean).length)).toEqual([3, 2, 2]);
    const tips = Array.from(container.querySelectorAll("rect title")).map((title) => title.textContent ?? "");
    expect(tips[2]).toMatch(/^07\/01\/2026 · PV /);
    expect(tips[2]).not.toMatch(/EV|AC/);
    expect(tips[1]).toMatch(/EV .* · AC /);
    expect(screen.getByText("EV (đạt được)")).toBeInTheDocument();
    expect(screen.getAllByText("Ngày trạng thái")).toHaveLength(1);
  });

  it("abbreviates large axis money in Vietnamese", () => {
    expect(formatCompactMoney(8_400_000)).toBe("8,4 Tr");
  });
});

describe("EvmPanel in English", () => {
  afterEach(() => setCurrentLang("vi"));

  function renderEnglish(metrics: ProjectMetrics | null) {
    const handlers = { onOpenProjectInfo: vi.fn(), onOpenBaseline: vi.fn(), onClose: vi.fn() };
    const view = render(
      <I18nProvider lang="en">
        <EvmPanel metrics={metrics} {...handlers} />
      </I18nProvider>,
    );
    return { ...view, user: userEvent.setup(), ...handlers };
  }

  it("shows the loading note", () => {
    renderEnglish(null);
    expect(screen.getByText("Loading figures…")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Enlarge" })).toBeDisabled();
  });

  it("names every metric with its abbreviation and formats numbers the English way", () => {
    renderEnglish(makeMetrics({ status_date: at(2026, 1, 15) }));
    expect(screen.getByRole("complementary", { name: "EVM metrics and burn-down" })).toBeInTheDocument();
    expect(screen.getByText("EVM metrics")).toBeInTheDocument();
    expect(screen.getByText("As of the status date 2026-01-15.")).toBeInTheDocument();
    expect(cell("Planned value (PV, BCWS)")).toHaveTextContent("4,000,000");
    expect(cell("Earned value (EV, BCWP)")).toHaveTextContent("3,000,000");
    expect(cell("Actual cost (AC, ACWP)")).toHaveTextContent("3,500,000");
    expect(cell("Schedule variance (SV)")).toHaveTextContent("-1,000,000");
    expect(cell("Cost variance (CV)")).toHaveClass("bad");
    expect(cell("Schedule performance index (SPI)")).toHaveTextContent("0.75");
    expect(cell("Cost performance index (CPI)")).toHaveTextContent("0.86");
    expect(cell("Budget at completion (BAC)")).toHaveTextContent("10,000,000");
    expect(cell("Estimate at completion (EAC)")).toHaveTextContent("11,666,667");
    expect(cell("Estimate to complete (ETC)")).toHaveTextContent("8,166,667");
    expect(cell("Variance at completion (VAC)")).toHaveTextContent("-1,666,667");
  });

  it("explains missing baseline and status date, with English buttons", async () => {
    const { user, onOpenBaseline, onOpenProjectInfo, onClose } = renderEnglish(
      makeMetrics({ status_date: null, has_baseline: false, metrics: null }),
    );
    expect(screen.getByText(/There is no baseline 0 yet/)).toBeInTheDocument();
    expect(screen.getByText(/There is no status date yet/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Set baseline…" }));
    await user.click(screen.getByRole("button", { name: "Set status date…" }));
    await user.click(screen.getByRole("button", { name: "Close the EVM panel" }));
    expect(onOpenBaseline).toHaveBeenCalledOnce();
    expect(onOpenProjectInfo).toHaveBeenCalledOnce();
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("labels the burn-down chart, axis, tooltip and legend", () => {
    const { container } = renderEnglish(
      makeMetrics(
        {},
        {
          status_date: at(2026, 1, 9),
          baseline_work: 4800,
          current_remaining_work: 1500,
          ideal: [
            { date: at(2026, 1, 5), remaining_work: 4800 },
            { date: at(2026, 1, 16), remaining_work: 0 },
          ],
          planned: [
            { date: at(2026, 1, 5), remaining_work: 4800 },
            { date: at(2026, 1, 16), remaining_work: 0 },
          ],
          actual: [{ date: at(2026, 1, 9), remaining_work: 1500, ev: 0, ac: 0 }],
        },
      ),
    );
    expect(screen.getByText("Remaining work now: 25 hrs · Baseline work: 80 hrs")).toBeInTheDocument();
    const chart = screen.getByRole("img", { name: "Burn-down chart: remaining work by date" });
    expect(within(chart).getByText("hrs")).toBeInTheDocument();
    expect(within(chart).getByText("Jan 5")).toBeInTheDocument();
    expect(container.querySelector("circle title")).toHaveTextContent("2026-01-09: 25 hrs left");
    expect(screen.getByText("Ideal (baseline 0)")).toBeInTheDocument();
    expect(screen.getByText("Scheduled")).toBeInTheDocument();
    expect(screen.getByText("Actual")).toBeInTheDocument();
    expect(screen.getByText("Status date")).toBeInTheDocument();
  });

  it("explains an empty burn-down and S-curve", () => {
    renderEnglish(makeMetrics());
    expect(screen.getByText(/^No data yet\./)).toBeInTheDocument();
    expect(screen.getByText("S-curve")).toBeInTheDocument();
    expect(screen.getByText("Set baseline 0 to draw the S-curve.")).toBeInTheDocument();
  });

  it("draws the S-curve with English legend and compact money", () => {
    renderEnglish(
      makeMetrics(
        {},
        {},
        {
          status_date: at(2026, 1, 6),
          baseline_finish: at(2026, 1, 7),
          step: "week",
          points: [
            { date: at(2026, 1, 5), pv: 0, ev: 0, ac: 0 },
            { date: at(2026, 1, 7), pv: 8_400_000, ev: null, ac: null },
          ],
        },
      ),
    );
    const chart = screen.getByRole("img", { name: "S-curve: cumulative PV, EV and AC over time" });
    expect(within(chart).getByText("8.4M")).toBeInTheDocument();
    expect(within(chart).getByText("4.2M")).toBeInTheDocument();
    expect(screen.getByText(/^Cumulative values by week\./)).toBeInTheDocument();
    expect(screen.getByText("PV (planned)")).toBeInTheDocument();
    expect(screen.getByText("EV (earned)")).toBeInTheDocument();
    expect(screen.getByText("AC (actual)")).toBeInTheDocument();
  });

  it("opens the enlarged charts dialog with an English title", async () => {
    const { user } = renderEnglish(makeMetrics());
    await user.click(screen.getByRole("button", { name: "Enlarge" }));
    expect(screen.getByRole("dialog", { name: "Burn-down and S-curve" })).toBeInTheDocument();
  });

  it("abbreviates thousands with K", () => {
    setCurrentLang("en");
    expect(formatCompactMoney(120_000)).toBe("120K");
    expect(formatWorkHours(90)).toBe("1.5 hrs");
    expect(formatTickDate(at(2026, 9, 14))).toBe("Sep 14");
  });
});
