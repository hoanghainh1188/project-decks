import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { I18nProvider } from "../i18n";
import { makeResource } from "../test/fixtures";
import type { AvailabilityPeriod, Command, Resource } from "../types";
import { ResourceAvailabilityDialog } from "./ResourceAvailabilityDialog";

function renderDialog(resource: Resource, { error = null, lang = "vi" }: { error?: string | null; lang?: "vi" | "en" } = {}) {
  const send = vi.fn().mockResolvedValue({});
  const onSave = vi.fn();
  const onCancel = vi.fn();
  render(
    <I18nProvider lang={lang}>
      <ResourceAvailabilityDialog
        resource={resource}
        changed={false}
        error={error}
        today="2026-01-05"
        onSave={onSave}
        onCancel={onCancel}
        send={send}
      />
    </I18nProvider>,
  );
  return { send, onSave, onCancel, user: userEvent.setup() };
}

const LEAVE: AvailabilityPeriod = { from: "2026-01-12", to: "2026-01-16", units: 0 };
const HALF: AvailabilityPeriod = { from: "2026-02-02", to: null, units: 0.5 };
const an = (availability: AvailabilityPeriod[] = []) => makeResource({ uid: 3, name: "An", max_units: 1, availability });

describe("ResourceAvailabilityDialog", () => {
  it("explains the max units and says when there is no period", () => {
    renderDialog(an());
    expect(screen.getByRole("dialog", { name: "Khả dụng: An" })).toBeInTheDocument();
    expect(screen.getByText(/Ngoài các khoảng dưới đây, An có đơn vị tối đa 1/)).toBeInTheDocument();
    expect(screen.getByText("Chưa có khoảng nào.")).toBeInTheDocument();
  });

  it("adds a week at half time from today, then after the last period", async () => {
    const { send, user } = renderDialog(an());
    await user.click(screen.getByRole("button", { name: "Thêm khoảng" }));
    expect(send).toHaveBeenLastCalledWith({
      type: "set_resource_availability",
      uid: 3,
      periods: [{ from: "2026-01-05", to: "2026-01-11", units: 0.5 }],
    });
  });

  it("starts a new period the day after the last one", async () => {
    const { send, user } = renderDialog(an([LEAVE]));
    await user.click(screen.getByRole("button", { name: "Thêm khoảng" }));
    expect(send).toHaveBeenLastCalledWith({
      type: "set_resource_availability",
      uid: 3,
      periods: [LEAVE, { from: "2026-01-17", to: "2026-01-23", units: 0.5 }],
    });
  });

  it("cannot add after a period with no end", () => {
    renderDialog(an([HALF]));
    expect(screen.getByRole("button", { name: "Thêm khoảng" })).toBeDisabled();
  });

  it("edits the units, also as hours per day, and shows them as hours", async () => {
    const { send, user } = renderDialog(an([LEAVE]));
    expect(screen.getByText("0 giờ/ngày")).toBeInTheDocument();
    const units = screen.getByRole("textbox", { name: "Đơn vị, khoảng 1" });
    await user.clear(units);
    await user.type(units, "2h/ngày");
    await user.tab();
    expect(send).toHaveBeenLastCalledWith({
      type: "set_resource_availability",
      uid: 3,
      periods: [{ ...LEAVE, units: 0.25 }],
    });
  });

  it("puts back units it cannot read", async () => {
    const { send, user } = renderDialog(an([LEAVE]));
    const units = screen.getByRole("textbox", { name: "Đơn vị, khoảng 1" });
    await user.clear(units);
    await user.type(units, "nhiều");
    await user.tab();
    expect(send).not.toHaveBeenCalled();
    expect(units).toHaveValue("0");
  });

  it("clears an end to make it open, and removes a period", async () => {
    const { send, user } = renderDialog(an([LEAVE, HALF]));
    expect(screen.getByRole("textbox", { name: "Đến ngày, khoảng 2" })).toHaveAttribute("placeholder", "Không giới hạn");
    const to = screen.getByRole("textbox", { name: "Đến ngày, khoảng 1" });
    await user.clear(to);
    await user.tab();
    expect(send).toHaveBeenLastCalledWith({
      type: "set_resource_availability",
      uid: 3,
      periods: [{ ...LEAVE, to: null }, HALF],
    });
    await user.click(screen.getByRole("button", { name: "Bỏ khoảng 2" }));
    // Built on the change before it.
    expect(send).toHaveBeenLastCalledWith({
      type: "set_resource_availability",
      uid: 3,
      periods: [{ ...LEAVE, to: null }],
    });
  });

  it("shows a refusal in the footer, and saves or cancels", async () => {
    const { onSave, onCancel, user } = renderDialog(an([LEAVE]), {
      error: "Các khoảng khả dụng của một nguồn lực không được trùng ngày",
    });
    expect(screen.getByRole("alert")).toHaveTextContent("không được trùng ngày");
    await user.click(screen.getByRole("button", { name: "Lưu" }));
    expect(onSave).toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Hủy" }));
    expect(onCancel).toHaveBeenCalled();
  });

  it("aims every quick change at the period it was made on", async () => {
    const B: AvailabilityPeriod = { from: "2026-02-02", to: "2026-02-06", units: 0.5 };
    const C: AvailabilityPeriod = { from: "2026-03-02", to: "2026-03-06", units: 2 };
    // The engine has not answered yet, so the rows still show all three.
    const send = vi.fn((_command: Command) => new Promise<undefined>(() => {}));
    render(
      <I18nProvider lang="vi">
        <ResourceAvailabilityDialog
          resource={an([LEAVE, B, C])}
          changed={false}
          error={null}
          today="2026-01-05"
          onSave={vi.fn()}
          onCancel={vi.fn()}
          send={send}
        />
      </I18nProvider>,
    );
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Bỏ khoảng 1" }));
    await user.click(screen.getByRole("button", { name: "Bỏ khoảng 2" }));
    const units = screen.getByRole("textbox", { name: "Đơn vị, khoảng 3" });
    await user.clear(units);
    await user.type(units, "1");
    await user.tab();
    const sent = send.mock.calls.map(([command]) => (command as { periods: AvailabilityPeriod[] }).periods);
    expect(sent).toEqual([[B, C], [C], [{ ...C, units: 1 }]]);
  });

  it("saves only once the last edit is in, and stays open when it was refused", async () => {
    const send = vi.fn().mockResolvedValue(undefined);
    const onSave = vi.fn();
    render(
      <I18nProvider lang="vi">
        <ResourceAvailabilityDialog
          resource={an([LEAVE])}
          changed={false}
          error={null}
          today="2026-01-05"
          onSave={onSave}
          onCancel={vi.fn()}
          send={send}
        />
      </I18nProvider>,
    );
    const user = userEvent.setup();
    const units = screen.getByRole("textbox", { name: "Đơn vị, khoảng 1" });
    await user.clear(units);
    await user.type(units, "0,5");
    // Clicking Lưu commits the field; the engine refuses it.
    await user.click(screen.getByRole("button", { name: "Lưu" }));
    expect(send).toHaveBeenCalledTimes(1);
    expect(onSave).not.toHaveBeenCalled();
    send.mockResolvedValue({});
    await user.click(screen.getByRole("button", { name: "Lưu" }));
    expect(onSave).toHaveBeenCalled();
  });

  it("speaks English", () => {
    renderDialog(an([LEAVE]), { lang: "en" });
    expect(screen.getByRole("dialog", { name: "Availability: An" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Add period" })).toBeInTheDocument();
    expect(screen.getByText("0 hrs/day")).toBeInTheDocument();
  });
});
