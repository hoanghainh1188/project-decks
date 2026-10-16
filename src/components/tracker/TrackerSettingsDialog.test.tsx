import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { applyCommand } from "../../api";
import { fakeBackend } from "../../test/fakeBackend";
import { makeProject, makeSnapshot } from "../../test/fixtures";
import { DEFAULT_SYNC_SETTINGS, type Command, type Project, type Source } from "../../types";
import { TrackerSettingsDialog } from "./TrackerSettingsDialog";

const SOURCE: Source = {
  kind: "backlog",
  site: "team.backlog.com",
  project_key: "PROJ",
  filter: "",
  hours_per_point: 4,
  status_map: [],
  sync: DEFAULT_SYNC_SETTINGS,
};

function projectWith(source: Source | null): Project {
  return makeProject({ fields: [{ id: 3, name: "Mảng", kind: { type: "text" } }], source });
}

function renderDialog(project: Project) {
  fakeBackend.reset(makeSnapshot(project), undefined);
  const send = vi.fn((command: Command) => applyCommand(command));
  const onClose = vi.fn();
  render(<TrackerSettingsDialog project={project} error={null} send={send} onClose={onClose} />);
  return { send, onClose, user: userEvent.setup() };
}

const saveButton = () => screen.getByRole("button", { name: "Lưu" });

describe("TrackerSettingsDialog", () => {
  it("says so when the project was never pulled from a tracker", () => {
    renderDialog(projectWith(null));
    expect(screen.getByText(/chưa kéo từ task tracker nào/)).toBeInTheDocument();
    expect(saveButton()).toBeDisabled();
  });

  it("lists every field with its defaults, including the project's custom fields", () => {
    renderDialog(projectWith(SOURCE));
    expect(screen.getByRole("combobox", { name: "Hạn: Hướng" })).toHaveValue("both");
    expect(screen.getByRole("combobox", { name: "Tên: Hướng" })).toHaveValue("pull");
    expect(screen.getByRole("combobox", { name: "Hạn: Khi xung đột" })).toHaveValue("ask");
    expect(screen.getByRole("combobox", { name: "Trường tùy chỉnh: Mảng: Hướng" })).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "Hạn: Đọc từ (ở nguồn)" })).toHaveAttribute("placeholder", "dueDate");
    expect(screen.getByRole("textbox", { name: "Bắt đầu dự kiến (ngày Sheafly tính): Ghi vào (ở nguồn)" })).toBeInTheDocument();
    // Fields with no direction to choose or nothing to read have no such control.
    expect(screen.queryByRole("textbox", { name: "Tên: Đọc từ (ở nguồn)" })).not.toBeInTheDocument();
    expect(screen.getByLabelText("Phần trăm hoàn thành của việc đang làm")).toHaveValue("50");
    expect(screen.getByLabelText("Giờ của ngày bắt đầu")).toHaveValue("8");
    expect(screen.getByLabelText("Giờ của hạn")).toHaveValue("17");
  });

  it("shows Jira's own defaults: no standard start date, the original estimate, duedate", () => {
    renderDialog(projectWith({ ...SOURCE, kind: "jira", site: "team.atlassian.net" }));
    expect(screen.getByRole("textbox", { name: "Hạn: Đọc từ (ở nguồn)" })).toHaveAttribute("placeholder", "duedate");
    expect(screen.getByRole("textbox", { name: "Effort: Đọc từ (ở nguồn)" })).toHaveAttribute("placeholder", "originalEstimate");
    expect(screen.getByRole("textbox", { name: "Ngày bắt đầu: Đọc từ (ở nguồn)" })).toHaveAttribute("placeholder", "custom:…");
    expect(screen.getByText(/Jira không có ngày bắt đầu chuẩn/)).toBeInTheDocument();
  });

  it("saves the defaults as a source with no policies", async () => {
    const { user, send, onClose } = renderDialog(projectWith(SOURCE));
    await user.click(saveButton());
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(send).toHaveBeenCalledWith({ type: "set_source", source: SOURCE });
  });

  it("saves the changes made as policies and constants", async () => {
    const { user, send, onClose } = renderDialog(projectWith(SOURCE));
    await user.selectOptions(screen.getByRole("combobox", { name: "Hạn: Hướng" }), "pull");
    await user.selectOptions(screen.getByRole("combobox", { name: "Hạn: Khi xung đột" }), "source");
    await user.type(screen.getByRole("textbox", { name: "Effort: Đọc từ (ở nguồn)" }), "custom:Điểm");
    await user.type(screen.getByRole("textbox", { name: "Bắt đầu dự kiến (ngày Sheafly tính): Ghi vào (ở nguồn)" }), "custom:Kế hoạch");
    await user.selectOptions(screen.getByRole("combobox", { name: "Trường tùy chỉnh: Mảng: Hướng" }), "off");
    const percent = screen.getByLabelText("Phần trăm hoàn thành của việc đang làm");
    await user.clear(percent);
    await user.type(percent, "30");
    const perPoint = screen.getByLabelText("Giờ mỗi điểm");
    await user.clear(perPoint);
    await user.type(perPoint, "2,5");
    await user.click(saveButton());
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    const saved = (send.mock.calls[0][0] as Extract<Command, { type: "set_source" }>).source!;
    expect(saved.hours_per_point).toBe(2.5);
    expect(saved.sync.in_progress_percent).toBe(30);
    expect(saved.sync.fields).toEqual([
      { field: "work", direction: "pull", on_conflict: "ask", source_field: "custom:Điểm", push_field: null },
      { field: "due", direction: "pull", on_conflict: "source", source_field: null, push_field: null },
      { field: "planned_start", direction: "push", on_conflict: "ask", source_field: null, push_field: "custom:Kế hoạch" },
      { field: "field:3", direction: "off", on_conflict: "ask", source_field: null, push_field: null },
    ]);
  });

  it("will not save numbers out of range", async () => {
    const { user } = renderDialog(projectWith(SOURCE));
    const percent = screen.getByLabelText("Phần trăm hoàn thành của việc đang làm");
    await user.clear(percent);
    await user.type(percent, "100");
    expect(saveButton()).toBeDisabled();
    expect(screen.getByText(/Phần trăm từ 1 đến 99/)).toBeInTheDocument();
    await user.clear(percent);
    await user.type(percent, "99");
    const hour = screen.getByLabelText("Giờ của hạn");
    await user.clear(hour);
    await user.type(hour, "24");
    expect(saveButton()).toBeDisabled();
  });

  it("checks the named fields against the source without saving", async () => {
    const { user, send } = renderDialog(projectWith(SOURCE));
    await user.type(screen.getByRole("textbox", { name: "Hạn: Đọc từ (ở nguồn)" }), "custom:Hạn thật");
    await user.click(screen.getByRole("button", { name: "Kiểm tra với nguồn" }));
    expect(await screen.findByText("Các trường khớp với dự án ở nguồn.")).toBeInTheDocument();
    const call = fakeBackend.calls.find((item) => item.cmd === "tracker_test");
    expect((call?.args?.source as Source).sync.fields[0]).toMatchObject({ field: "due", source_field: "custom:Hạn thật" });
    expect(send).not.toHaveBeenCalled();
  });

  it("shows the source's refusal of a field", async () => {
    const { user } = renderDialog(projectWith(SOURCE));
    fakeBackend.failNext("tracker_test", "Một trường trong thiết lập đồng bộ không có ở nguồn");
    await user.click(screen.getByRole("button", { name: "Kiểm tra với nguồn" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("không có ở nguồn");
  });

  it("resets the rows and the constants to the defaults", async () => {
    const saved: Source = {
      ...SOURCE,
      sync: { fields: [{ field: "due", direction: "off", on_conflict: "ask", source_field: null, push_field: null }], in_progress_percent: 30, start_hour: 9, due_hour: 18 },
    };
    const { user } = renderDialog(projectWith(saved));
    expect(screen.getByRole("combobox", { name: "Hạn: Hướng" })).toHaveValue("off");
    await user.click(screen.getByRole("button", { name: "Đặt lại mặc định" }));
    expect(screen.getByRole("combobox", { name: "Hạn: Hướng" })).toHaveValue("both");
    expect(screen.getByLabelText("Phần trăm hoàn thành của việc đang làm")).toHaveValue("50");
    expect(screen.getByLabelText("Giờ của ngày bắt đầu")).toHaveValue("8");
  });

  it("shows an error the engine gives and stays open", async () => {
    const project = projectWith(SOURCE);
    fakeBackend.reset(makeSnapshot(project), undefined);
    const send = vi.fn(async () => undefined);
    const onClose = vi.fn();
    render(<TrackerSettingsDialog project={project} error="Chính sách đồng bộ không hợp lệ" send={send} onClose={onClose} />);
    expect(screen.getByRole("alert")).toHaveTextContent("Chính sách đồng bộ không hợp lệ");
    fireEvent.click(saveButton());
    await waitFor(() => expect(send).toHaveBeenCalled());
    expect(onClose).not.toHaveBeenCalled();
  });
});
