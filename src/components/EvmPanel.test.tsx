import { fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { at, makeEvm, makeMetrics } from "../test/fixtures";
import type { ProjectMetrics } from "../types";
import { EvmPanel } from "./EvmPanel";

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
});
