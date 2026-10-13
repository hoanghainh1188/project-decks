import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { makeProject } from "../test/fixtures";
import { I18nProvider } from "../i18n";
import { ProjectDialog } from "./ProjectDialog";

function renderDialog(editable: boolean) {
  const send = vi.fn().mockResolvedValue(undefined);
  render(
    <I18nProvider lang="vi">
      <ProjectDialog
        project={makeProject({ editable_actual_costs: editable })}
        changed={false}
        onSave={vi.fn()}
        onCancel={vi.fn()}
        send={send}
        onCreateCalendar={vi.fn()}
      />
    </I18nProvider>,
  );
  return send;
}

describe("ProjectDialog actual costs", () => {
  it("turns hand-entered actual costs on", async () => {
    const send = renderDialog(false);
    const box = screen.getByRole("checkbox", { name: "Nhập chi phí thực tế bằng tay" });
    expect(box).not.toBeChecked();
    await userEvent.setup().click(box);
    expect(send).toHaveBeenCalledWith({ type: "set_editable_actual_costs", editable: true });
  });

  it("shows the switch on and turns it off", async () => {
    const send = renderDialog(true);
    const box = screen.getByRole("checkbox", { name: "Nhập chi phí thực tế bằng tay" });
    expect(box).toBeChecked();
    await userEvent.setup().click(box);
    expect(send).toHaveBeenCalledWith({ type: "set_editable_actual_costs", editable: false });
  });
});
