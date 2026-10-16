import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { I18nProvider } from "../i18n";
import { makeProject } from "../test/fixtures";
import type { FieldDef } from "../types";
import { ForecastTab } from "./ForecastTab";

const FIELDS: FieldDef[] = [
  { id: 1, name: "Mảng", kind: { type: "choice", options: ["BE", "FE"] } },
  { id: 2, name: "Điểm", kind: { type: "number" } },
  { id: 3, name: "Ghi chú", kind: { type: "text" } },
];

function renderTab(lang: "vi" | "en" = "vi") {
  const send = vi.fn().mockResolvedValue(undefined);
  const project = makeProject({ fields: FIELDS });
  render(
    <I18nProvider lang={lang}>
      <ForecastTab project={project} send={send} />
    </I18nProvider>,
  );
  return { send };
}

describe("ForecastTab", () => {
  it("groups by a text or choice field only and sends the whole settings", () => {
    const { send } = renderTab();
    const field = screen.getByLabelText("Tương quan theo");
    const names = [...field.querySelectorAll("option")].map((option) => option.textContent);
    expect(names).toEqual(["Không theo trường nào", "Mảng", "Ghi chú"]);
    fireEvent.change(field, { target: { value: "1" } });
    expect(send).toHaveBeenLastCalledWith({ type: "set_forecast_settings", settings: { field: 1, group: "medium", common: "low" } });
  });

  it("sets how strongly tasks move together", () => {
    const { send } = renderTab();
    // Without a field the group strength means nothing, so it waits for one.
    expect(screen.getByLabelText("Mức trong nhóm")).toBeDisabled();
    fireEvent.change(screen.getByLabelText("Mức chung"), { target: { value: "high" } });
    expect(send).toHaveBeenLastCalledWith({ type: "set_forecast_settings", settings: { field: null, group: "medium", common: "high" } });
  });

  it("speaks English", () => {
    renderTab("en");
    expect(screen.getByLabelText("Correlate by")).toBeInTheDocument();
    expect(screen.getAllByRole("option", { name: "High" })).toHaveLength(2);
  });
});
