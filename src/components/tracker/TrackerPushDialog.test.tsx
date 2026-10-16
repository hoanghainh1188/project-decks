import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { applyCommand } from "../../api";
import { fakeBackend } from "../../test/fakeBackend";
import { makeProject, makeSnapshot, makeTask } from "../../test/fixtures";
import { DEFAULT_SYNC_SETTINGS } from "../../types";
import type { Command, Project, PushPreview, PushReport, Source } from "../../types";
import { TrackerPushDialog } from "./TrackerPushDialog";

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
  return makeProject({
    tasks: [
      makeTask({ uid: 1, name: "Màn hình đăng nhập", external: { key: "PROJ-1", synced_at: 1 } }),
      makeTask({ uid: 2, name: "Kiểm tra mật khẩu", external: { key: "PROJ-2", synced_at: 1 } }),
    ],
    next_task_uid: 3,
    source,
  });
}

const PREVIEW: PushPreview = {
  plan: {
    changes: [
      {
        uid: 1,
        key: "PROJ-1",
        name: "Màn hình đăng nhập",
        start: { from: null, to: "2026-01-05" },
        due: { from: "2026-01-09", to: "2026-01-06" },
        assignee: null,
      },
      {
        uid: 2,
        key: "PROJ-2",
        name: "Kiểm tra mật khẩu",
        start: null,
        due: { from: null, to: "2026-01-07" },
        assignee: { from: null, to: "an@example.com", resource_uid: 10 },
      },
    ],
    skipped: [
      { uid: 3, key: "PROJ-3", name: "Việc nhóm", reason: "many_assignees" },
      { uid: 1, key: "PROJ-1", name: "Màn hình đăng nhập", reason: "source_changed", field: "due" },
    ],
    missing: [{ uid: 4, key: "PROJ-4", name: "Việc mất" }],
  },
  unknown_assignees: [],
};

const REPORT: PushReport = {
  done: ["PROJ-1", "PROJ-2"],
  entries: [
    { key: "PROJ-1", start: "2026-01-05", due: "2026-01-06" },
    { key: "PROJ-2", due: "2026-01-07" },
  ],
  failed: [],
  not_sent: [],
  stopped: null,
};

function renderDialog(project: Project, preview: PushPreview | null = PREVIEW, keys = ["team.backlog.com"]) {
  fakeBackend.reset(makeSnapshot(project), undefined);
  fakeBackend.pushPreview = preview;
  fakeBackend.pushReport = REPORT;
  for (const site of keys) fakeBackend.trackerKeys.add(site);
  const send = vi.fn((command: Command) => applyCommand(command));
  const onClose = vi.fn();
  const view = render(<TrackerPushDialog project={project} error={null} send={send} onClose={onClose} />);
  return { send, onClose, view, user: userEvent.setup() };
}

const calls = (cmd: string) => fakeBackend.calls.filter((call) => call.cmd === cmd);
const previewButton = () => screen.getByRole("button", { name: "Xem trước" });

async function previewed(user: ReturnType<typeof userEvent.setup>) {
  await waitFor(() => expect(previewButton()).toBeEnabled());
  await user.click(previewButton());
  await screen.findByRole("table", { name: "Việc sẽ ghi" });
}

describe("TrackerPushDialog", () => {
  it("says so when the project was never pulled from a tracker", () => {
    renderDialog(projectWith(null));
    expect(screen.getByText(/chưa kéo từ task tracker nào/)).toBeInTheDocument();
    expect(previewButton()).toBeDisabled();
  });

  it("needs a saved key, and at least one thing to write", async () => {
    renderDialog(projectWith(SOURCE), PREVIEW, []);
    expect(await screen.findByText(/Chưa có khóa API cho team.backlog.com/)).toBeInTheDocument();
    expect(previewButton()).toBeDisabled();
  });

  it("starts with the dates on, the assignee off and only empty fields filled", async () => {
    renderDialog(projectWith(SOURCE));
    expect(screen.getByRole("checkbox", { name: /Ngày bắt đầu của việc/ })).toBeChecked();
    expect(screen.getByRole("checkbox", { name: /Ngày kết thúc của việc/ })).toBeChecked();
    expect(screen.getByRole("checkbox", { name: /Người được giao/ })).not.toBeChecked();
    expect(screen.getByRole("radio", { name: "Chỉ điền ô trống" })).toBeChecked();
    expect(screen.getByText(/hạn ở nguồn được đọc thành hạn chót/)).toBeInTheDocument();
    await waitFor(() => expect(previewButton()).toBeEnabled());
  });

  it("cannot preview with nothing chosen", async () => {
    const { user } = renderDialog(projectWith(SOURCE));
    await user.click(screen.getByRole("checkbox", { name: /Ngày bắt đầu của việc/ }));
    await user.click(screen.getByRole("checkbox", { name: /Ngày kết thúc của việc/ }));
    expect(screen.getByText("Chọn ít nhất một thứ để ghi.")).toBeInTheDocument();
    expect(previewButton()).toBeDisabled();
  });

  it("previews what would change at the source with the options chosen", async () => {
    const { user } = renderDialog(projectWith(SOURCE));
    await user.click(screen.getByRole("checkbox", { name: /Người được giao/ }));
    await user.click(screen.getByRole("radio", { name: "Ghi đè giá trị khác" }));
    await previewed(user);
    expect(calls("tracker_push_preview")[0].args).toEqual({
      source: SOURCE,
      options: { start: true, due: true, assignee: true, overwrite: true },
    });
    const table = screen.getByRole("table", { name: "Việc sẽ ghi" });
    expect(within(table).getByText("— → 2026-01-05")).toBeInTheDocument();
    expect(within(table).getByText("2026-01-09 → 2026-01-06")).toBeInTheDocument();
    expect(within(table).getByText("— → an@example.com")).toBeInTheDocument();
    expect(screen.getByText(/Sẽ ghi lên Backlog/)).toBeInTheDocument();
    expect(within(screen.getByRole("list", { name: "Bỏ qua" })).getByText("PROJ-3: có nhiều người, nguồn chỉ nhận một")).toBeInTheDocument();
    expect(
      within(screen.getByRole("list", { name: "Bỏ qua" })).getByText("PROJ-1: hạn đã đổi ở nguồn từ lần đồng bộ trước; hãy kéo và giải quyết trước"),
    ).toBeInTheDocument();
    expect(screen.getByText(/1 việc đã liên kết nhưng nguồn không trả về/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Ghi 2 việc lên Backlog" })).toBeEnabled();
    // Nothing was written by previewing.
    expect(calls("tracker_push")).toHaveLength(0);
  });

  it("warns about people the source does not know", async () => {
    const { user } = renderDialog(projectWith(SOURCE), { ...PREVIEW, unknown_assignees: ["la@example.com"] });
    await previewed(user);
    expect(screen.getByRole("alert")).toHaveTextContent("la@example.com");
  });

  it("writes only after the button, then remembers the days that went through", async () => {
    const { user, send } = renderDialog(projectWith(SOURCE));
    await previewed(user);
    fireEvent.click(screen.getByRole("button", { name: "Ghi 2 việc lên Backlog" }));
    expect(await screen.findByText("Đã ghi 2 việc.")).toBeInTheDocument();
    expect(calls("tracker_push")[0].args).toEqual({
      source: SOURCE,
      options: { start: true, due: true, assignee: false, overwrite: false },
    });
    expect(send).toHaveBeenCalledWith({ type: "record_push", entries: REPORT.entries });
    expect(screen.getByRole("button", { name: "Đóng" })).toBeEnabled();
  });

  it("reports what failed and what was not sent, and records only what went through", async () => {
    const { user, send } = renderDialog(projectWith(SOURCE));
    fakeBackend.pushReport = {
      done: ["PROJ-1"],
      entries: [{ key: "PROJ-1", start: "2026-01-05", due: "2026-01-06" }],
      failed: [{ key: "PROJ-2", message: "Nguồn đang giới hạn số yêu cầu; thử lại sau một phút" }],
      not_sent: ["PROJ-3", "PROJ-4"],
      stopped: "Nguồn đang giới hạn số yêu cầu; thử lại sau một phút",
    };
    await previewed(user);
    fireEvent.click(screen.getByRole("button", { name: /^Ghi 2 việc/ }));
    expect(await screen.findByText("Đã ghi 1 việc.")).toBeInTheDocument();
    expect(screen.getByText(/Dừng giữa chừng: Nguồn đang giới hạn/)).toBeInTheDocument();
    expect(within(screen.getByRole("list", { name: "1 việc lỗi:" })).getByText(/PROJ-2/)).toBeInTheDocument();
    expect(screen.getByText("2 việc chưa gửi.")).toBeInTheDocument();
    expect(send).toHaveBeenCalledWith({ type: "record_push", entries: [{ key: "PROJ-1", start: "2026-01-05", due: "2026-01-06" }] });
  });

  it("records nothing when no day went through", async () => {
    const { user, send } = renderDialog(projectWith(SOURCE));
    fakeBackend.pushReport = { done: ["PROJ-2"], entries: [], failed: [], not_sent: [], stopped: null };
    await previewed(user);
    fireEvent.click(screen.getByRole("button", { name: /^Ghi 2 việc/ }));
    await screen.findByText("Đã ghi 1 việc.");
    expect(send).not.toHaveBeenCalled();
  });

  it("offers nothing to write when the source already matches", async () => {
    const { user } = renderDialog(projectWith(SOURCE), { plan: { changes: [], skipped: [], missing: [] }, unknown_assignees: [] });
    await waitFor(() => expect(previewButton()).toBeEnabled());
    await user.click(previewButton());
    expect(await screen.findByText(/Không có gì để ghi/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Ghi 0 việc lên Backlog" })).toBeDisabled();
  });

  it("goes back to the options when the project changes under a preview", async () => {
    const project = projectWith(SOURCE);
    const { user, view, send } = renderDialog(project);
    await previewed(user);
    view.rerender(<TrackerPushDialog project={{ ...project, name: "Đổi tên" }} error={null} send={send} onClose={() => {}} />);
    expect(await screen.findByText("Dự án đã đổi từ lúc xem trước; hãy xem trước lại.")).toBeInTheDocument();
    expect(screen.queryByRole("table", { name: "Việc sẽ ghi" })).not.toBeInTheDocument();
  });

  it("shows an error from the source and a cancelled preview returns to the options", async () => {
    const { user } = renderDialog(projectWith(SOURCE), null);
    await waitFor(() => expect(previewButton()).toBeEnabled());
    await user.click(previewButton());
    await waitFor(() => expect(previewButton()).toBeEnabled());
    expect(screen.queryByRole("table", { name: "Việc sẽ ghi" })).not.toBeInTheDocument();
    fakeBackend.failNext("tracker_push_preview", "Site không nhận khóa API này");
    await user.click(previewButton());
    expect(await screen.findByRole("alert")).toHaveTextContent("Site không nhận khóa API này");
  });

  it("cannot be closed while writing; only Stop ends it", async () => {
    const { user, onClose } = renderDialog(projectWith(SOURCE));
    await previewed(user);
    const release = fakeBackend.holdNext("tracker_push");
    fireEvent.click(screen.getByRole("button", { name: /^Ghi 2 việc/ }));
    await screen.findByRole("button", { name: "Dừng" });
    fireEvent.keyDown(document, { key: "Escape" });
    expect(onClose).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Dừng" }));
    expect(calls("tracker_cancel")).toHaveLength(1);
    release();
    await screen.findByText("Đã ghi 2 việc.");
  });
});
