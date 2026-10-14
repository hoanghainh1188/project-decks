import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { I18nProvider } from "../i18n";
import { makeProject, makeResource } from "../test/fixtures";
import type { Command, Snapshot } from "../types";
import { AssignmentDialog } from "./AssignmentDialog";

function renderDialog({
  error = null,
  changed = false,
  send = vi.fn().mockResolvedValue({}),
}: { error?: string | null; changed?: boolean; send?: ReturnType<typeof vi.fn> } = {}) {
  const handlers = { onSave: vi.fn(), onCancel: vi.fn(), send };
  const project = makeProject({ skills: [{ id: 1, name: "Web" }], resources: [makeResource({ uid: 10, name: "An" })] });
  render(
    <I18nProvider lang="vi">
      <AssignmentDialog
        project={project}
        changed={changed}
        error={error}
        onSave={handlers.onSave}
        onCancel={handlers.onCancel}
        send={send as unknown as (command: Command) => Promise<Snapshot | void>}
      />
    </I18nProvider>,
  );
  return { ...handlers, user: userEvent.setup() };
}

describe("AssignmentDialog", () => {
  it("opens on the rules and switches to the skills with the arrow keys", async () => {
    const { user } = renderDialog();
    expect(screen.getByRole("dialog", { name: "Giao việc" })).toBeInTheDocument();
    const rules = screen.getByRole("tab", { name: "Quy tắc" });
    expect(rules).toHaveAttribute("aria-selected", "true");
    rules.focus();
    await user.keyboard("{ArrowLeft}");
    expect(screen.getByRole("tab", { name: "Kỹ năng" })).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("textbox", { name: "Kỹ năng mới" })).toBeInTheDocument();
  });

  it("shows a refused edit in its footer and saves or cancels", async () => {
    const { onSave, onCancel, user } = renderDialog({ error: "Không có kỹ năng này" });
    expect(screen.getByRole("alert")).toHaveTextContent("Không có kỹ năng này");
    await user.click(screen.getByRole("button", { name: "Lưu" }));
    await waitFor(() => expect(onSave).toHaveBeenCalled());
    await user.click(screen.getByRole("button", { name: "Hủy" }));
    expect(onCancel).toHaveBeenCalled();
  });

  it("saves only once the engine took the edits, and stays open when it refused one", async () => {
    let answer: (state: unknown) => void = () => {};
    const send = vi.fn().mockReturnValue(new Promise((resolve) => (answer = resolve)));
    const { onSave, user } = renderDialog({ send });
    await user.click(screen.getByRole("button", { name: "Thêm quy tắc" }));
    await user.click(screen.getByRole("button", { name: "Lưu" }));
    expect(onSave).not.toHaveBeenCalled();
    await act(async () => answer(undefined));
    expect(onSave).not.toHaveBeenCalled();
    // A later edit the engine takes clears the refusal: one click on Lưu closes.
    send.mockResolvedValue({});
    await user.click(screen.getByRole("button", { name: "Thêm quy tắc" }));
    await user.click(screen.getByRole("button", { name: "Lưu" }));
    await waitFor(() => expect(onSave).toHaveBeenCalledTimes(1));
  });

  it("says it is previewing once something changed", () => {
    renderDialog({ changed: true });
    expect(screen.getByText("Đang xem trước thay đổi")).toBeInTheDocument();
  });
});
