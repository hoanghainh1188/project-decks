import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import { I18nProvider, setCurrentLang } from "../i18n";
import { makeTask } from "../test/fixtures";
import { TaskEditDialog } from "./TaskEditDialog";

describe("TaskEditDialog", () => {
  it("sends the same commands as the task sheet when fields change", async () => {
    const user = userEvent.setup();
    const send = vi.fn();
    render(<TaskEditDialog task={makeTask({ uid: 4, name: "Cũ", predecessors: "" })} send={send} onClose={() => {}} />);
    const inputs = screen.getAllByRole("textbox");
    const name = inputs[0];
    await user.clear(name);
    await user.type(name, "Mới{Enter}");
    const duration = inputs[1];
    await user.clear(duration);
    await user.type(duration, "2d{Enter}");
    const predecessors = screen.getByPlaceholderText("VD: 2, 3SS+1d");
    await user.type(predecessors, "2SS{Enter}");
    expect(send).toHaveBeenCalledWith({ type: "rename_task", uid: 4, name: "Mới" });
    expect(send).toHaveBeenCalledWith({ type: "set_duration", uid: 4, minutes: 960 });
    expect(send).toHaveBeenCalledWith({ type: "set_predecessors", uid: 4, text: "2SS" });
  });

  it("closes on Escape", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    render(<TaskEditDialog task={makeTask({ uid: 1 })} send={() => {}} onClose={onClose} />);
    await user.keyboard("{Escape}");
    expect(onClose).toHaveBeenCalled();
  });

  it("locks schedule fields on summary tasks", () => {
    render(<TaskEditDialog task={makeTask({ uid: 1, summary: true })} send={() => {}} onClose={() => {}} />);
    expect(screen.getByDisplayValue("1 ngày")).toBeDisabled();
  });
});

describe("TaskEditDialog in English", () => {
  afterEach(() => setCurrentLang("vi"));

  it("labels the fields with Project's English terms", () => {
    render(
      <I18nProvider lang="en">
        <TaskEditDialog task={makeTask({ uid: 2, wbs: "1.2" })} send={() => {}} onClose={() => {}} />
      </I18nProvider>,
    );
    expect(screen.getByRole("heading", { name: "1.2 · Edit Task" })).toBeInTheDocument();
    for (const label of ["Name", "Duration", "Start", "Finish", "% Complete", "Predecessors"]) {
      expect(screen.getByText(label, { selector: "label" })).toBeInTheDocument();
    }
    expect(screen.getByDisplayValue("1 day")).toBeInTheDocument();
    expect(screen.getByPlaceholderText("e.g. 2, 3SS+1d")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Close" })).toBeInTheDocument();
  });
});
