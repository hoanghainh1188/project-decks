import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { applyCommand } from "../../api";
import { fakeBackend } from "../../test/fakeBackend";
import { makeProject, makeResource, makeSnapshot, makeTask } from "../../test/fixtures";
import { DEFAULT_SYNC_SETTINGS } from "../../types";
import type { Command, Project, PullPreview, Source } from "../../types";
import { TrackerDialog } from "./TrackerDialog";

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
    tasks: [makeTask({ uid: 1, name: "Mở đầu" })],
    resources: [makeResource({ uid: 10, name: "An" }), makeResource({ uid: 11, name: "Bình", email: "binh@example.com" })],
    next_task_uid: 2,
    source,
  });
}

const PREVIEW: PullPreview = {
  rows: [
    { key: "PROJ-1", name: "Màn hình đăng nhập", assignee_email: "an@example.com" },
    { key: "PROJ-2", name: "Kiểm tra mật khẩu", parent_key: "PROJ-1" },
    { key: "PROJ-3", name: "Việc cũ" },
  ],
  pulled_at: 123,
  warnings: [
    { key: "PROJ-9", kind: "no_email" },
    { key: "PROJ-2", kind: "field_value" },
  ],
  changes: [
    { row: 0, key: "PROJ-1", name: "Màn hình đăng nhập", uid: 2, kind: "add", fields: [] },
    { row: 1, key: "PROJ-2", name: "Kiểm tra mật khẩu", uid: 3, kind: "change", fields: ["name", "due"] },
    { row: 2, key: "PROJ-3", name: "Việc cũ", uid: 4, kind: "same", fields: [] },
  ],
  gone: [{ uid: 5, key: "PROJ-7", name: "Việc đã xóa" }],
  problems: [],
  conflicts: [],
  bases_changed: false,
};

function renderDialog(project: Project, preview: PullPreview | null = PREVIEW, keys: string[] = []) {
  fakeBackend.reset(makeSnapshot(project), undefined);
  fakeBackend.pullPreview = preview;
  for (const site of keys) fakeBackend.trackerKeys.add(site);
  const send = vi.fn((command: Command) => applyCommand(command));
  const onClose = vi.fn();
  const view = render(<TrackerDialog project={project} error={null} send={send} onClose={onClose} />);
  return { send, onClose, view, user: userEvent.setup() };
}

function fetchButton() {
  return screen.getByRole("button", { name: "Lấy dữ liệu" });
}

async function fetched(user: ReturnType<typeof userEvent.setup>) {
  await user.click(fetchButton());
  await screen.findByText(/^Thêm 1 · Đổi 1/);
}

const calls = (cmd: string) => fakeBackend.calls.filter((call) => call.cmd === cmd);

describe("TrackerDialog", () => {
  it("starts from the project's source and needs a site and a project key to fetch", async () => {
    const { user } = renderDialog(projectWith(null));
    expect(fetchButton()).toBeDisabled();
    await user.type(screen.getByLabelText(/^Site/), "Team.Backlog.com");
    expect(fetchButton()).toBeDisabled();
    await user.type(screen.getByLabelText("Khóa dự án ở nguồn"), "PROJ");
    expect(fetchButton()).toBeEnabled();
  });

  it("fills the form from the saved source and says whether a key is saved", async () => {
    const project = projectWith({ ...SOURCE, filter: "statusId[]=1", status_map: [{ status: "Đang review", class: "in_progress" }] });
    renderDialog(project, PREVIEW, ["team.backlog.com"]);
    expect(screen.getByLabelText(/^Site/)).toHaveValue("team.backlog.com");
    expect(screen.getByLabelText("Khóa dự án ở nguồn")).toHaveValue("PROJ");
    expect(screen.getByLabelText(/^Bộ lọc/)).toHaveValue("statusId[]=1");
    expect(screen.getByDisplayValue("Đang review")).toBeInTheDocument();
    expect(await screen.findByText(/Đã có khóa lưu trong kho khóa/)).toBeInTheDocument();
  });

  it("sends a typed key to the credential store once and never shows it again", async () => {
    const { user } = renderDialog(projectWith(SOURCE));
    await user.type(screen.getByLabelText("Khóa API"), "SECRET");
    await user.click(fetchButton());
    await screen.findByText(/^Thêm 1 · Đổi 1/);
    expect(calls("tracker_save_key")).toHaveLength(1);
    expect(calls("tracker_save_key")[0].args).toEqual({ site: "team.backlog.com", key: "SECRET" });
    expect(fakeBackend.trackerKeys.has("team.backlog.com")).toBe(true);
    await user.click(screen.getByRole("button", { name: "Quay lại" }));
    expect(screen.getByLabelText("Khóa API")).toHaveValue("");
    expect(await screen.findByText(/Đã có khóa lưu trong kho khóa/)).toBeInTheDocument();
    expect(document.body.textContent).not.toContain("SECRET");
  });

  it("forgets the key", async () => {
    const { user } = renderDialog(projectWith(SOURCE), PREVIEW, ["team.backlog.com"]);
    await user.click(await screen.findByRole("button", { name: "Xóa khóa" }));
    await waitFor(() => expect(screen.getByText(/Chưa có khóa/)).toBeInTheDocument());
    expect(fakeBackend.trackerKeys.has("team.backlog.com")).toBe(false);
  });

  it("tests the connection and names the project", async () => {
    const { user } = renderDialog(projectWith(SOURCE));
    await user.click(screen.getByRole("button", { name: "Kiểm tra kết nối" }));
    expect(await screen.findByText("Kết nối được: Dự án mẫu")).toBeInTheDocument();
  });

  it("previews what pulling does and applies it as the source then the pull", async () => {
    const { user, send, onClose } = renderDialog(projectWith({ ...SOURCE, filter: "" }));
    // Change the filter so the source differs from the saved one.
    await user.type(screen.getByLabelText(/^Bộ lọc/), "keyword=đăng nhập");
    await fetched(user);
    expect(screen.getByText("Thêm 1 · Đổi 1 · Không đổi 1 · Đã xóa ở nguồn 1")).toBeInTheDocument();
    const table = screen.getByRole("table", { name: "Việc sẽ thêm hoặc đổi" });
    expect(within(table).getByText("Thêm mới")).toBeInTheDocument();
    expect(within(table).getByText("tên, hạn")).toBeInTheDocument();
    expect(within(table).queryByText("Việc cũ")).not.toBeInTheDocument();
    expect(screen.getByText(/Việc đã xóa/)).toBeInTheDocument();
    expect(screen.getByText(/1 việc có người được giao nhưng nguồn không cho biết email/)).toBeInTheDocument();
    expect(screen.getByText(/1 việc có giá trị trường tùy chỉnh không phải số/)).toBeInTheDocument();
    const apply = screen.getByRole("button", { name: "Áp dụng 3 thay đổi" });
    fireEvent.click(apply);
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(send.mock.calls.map(([command]) => command.type)).toEqual(["set_source", "apply_pull"]);
    expect(send.mock.calls[0][0]).toMatchObject({ source: { site: "team.backlog.com", project_key: "PROJ", filter: "keyword=đăng nhập" } });
    expect(send.mock.calls[1][0]).toEqual({ type: "apply_pull", rows: PREVIEW.rows, pulled_at: 123, resolutions: [] });
  });

  it("saves no source when the form is the one the project holds", async () => {
    const { user, send, onClose } = renderDialog(projectWith(SOURCE));
    await fetched(user);
    fireEvent.click(screen.getByRole("button", { name: /^Áp dụng/ }));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(send.mock.calls.map(([command]) => command.type)).toEqual(["apply_pull"]);
  });

  it("does not apply while an issue has a problem, and matches an unknown email to a person", async () => {
    const withProblem: PullPreview = {
      ...PREVIEW,
      problems: [{ message: "Dự án không có người nào dùng email này", row: 0, key: "PROJ-1", column: "assignee" }],
    };
    const { user, send, view } = renderDialog(projectWith(SOURCE), withProblem);
    await fetched(user);
    expect(screen.getByRole("button", { name: /^Áp dụng/ })).toBeDisabled();
    expect(screen.getByText("Email chưa khớp người nào")).toBeInTheDocument();
    await user.selectOptions(screen.getByRole("combobox", { name: "Người có email an@example.com" }), "10");
    expect(send).toHaveBeenCalledWith({ type: "set_resource_email", uid: 10, email: "an@example.com" });
    // Only people without an email are offered.
    const options = within(screen.getByRole("combobox", { name: "Người có email an@example.com" })).getAllByRole("option");
    expect(options.map((option) => option.textContent)).toEqual(["Chọn người…", "An"]);

    // The project now has the email: the dialog asks the engine again and the problem is gone.
    fakeBackend.pullChanges = { changes: PREVIEW.changes, gone: PREVIEW.gone, problems: [], conflicts: [], bases_changed: false };
    const next = fakeBackend.snapshot.project;
    const before = calls("tracker_check").length;
    view.rerender(<TrackerDialog project={{ ...next }} error={null} send={send} onClose={() => {}} />);
    await waitFor(() => expect(calls("tracker_check").length).toBe(before + 1));
    await waitFor(() => expect(screen.getByRole("button", { name: /^Áp dụng/ })).toBeEnabled());
    expect(calls("tracker_check")[calls("tracker_check").length - 1]?.args).toEqual({ rows: PREVIEW.rows, pulledAt: 123, resolutions: [] });
  });

  const CONFLICTS: PullPreview = {
    ...PREVIEW,
    changes: PREVIEW.changes.map((row) => ({ ...row, kind: "same" as const, fields: [] })),
    gone: [],
    warnings: [],
    conflicts: [
      { row: 0, key: "PROJ-1", name: "Màn hình đăng nhập", uid: 2, field: "name", base: "Cũ", source: "Tên ở nguồn", local: "Tên của tôi" },
      { row: 1, key: "PROJ-2", name: "Kiểm tra mật khẩu", uid: 3, field: "work", base: null, source: "480", local: "180" },
      { row: 1, key: "PROJ-2", name: "Kiểm tra mật khẩu", uid: 3, field: "status", base: "not_started", source: "done", local: "in_progress" },
    ],
  };

  it("lists fields changed on both sides and blocks Apply until each has a choice", async () => {
    const { user } = renderDialog(projectWith(SOURCE), CONFLICTS);
    await user.click(fetchButton());
    const table = await screen.findByRole("table", { name: "Xung đột" });
    expect(screen.getByText("Xung đột: 3 chưa chọn")).toBeInTheDocument();
    expect(within(table).getByText("Tên ở nguồn")).toBeInTheDocument();
    expect(within(table).getByText("không rõ")).toBeInTheDocument();
    expect(within(table).getByText("8 giờ")).toBeInTheDocument();
    expect(within(table).getByText("3 giờ")).toBeInTheDocument();
    expect(within(table).getByText("Đang làm")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /^Áp dụng/ })).toBeDisabled();
    await user.click(screen.getByRole("radio", { name: "PROJ-1 Tên: Sheafly" }));
    await user.click(screen.getByRole("radio", { name: "PROJ-2 Effort: Nguồn" }));
    expect(screen.getByText("Xung đột: 1 chưa chọn")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /^Áp dụng/ })).toBeDisabled();
  });

  it("asks the engine again under the choices and applies them with the pull", async () => {
    const { user, send, onClose } = renderDialog(projectWith(SOURCE), CONFLICTS);
    fakeBackend.pullChanges = {
      changes: PREVIEW.changes,
      gone: PREVIEW.gone,
      problems: [],
      conflicts: [],
      bases_changed: false,
    };
    await user.click(fetchButton());
    await screen.findByRole("table", { name: "Xung đột" });
    await user.click(screen.getByRole("button", { name: "Lấy nguồn cho tất cả" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "Áp dụng 3 thay đổi" })).toBeEnabled());
    const last = calls("tracker_check")[calls("tracker_check").length - 1];
    expect(last.args).toMatchObject({
      resolutions: [
        { key: "PROJ-1", field: "name", keep: "source" },
        { key: "PROJ-2", field: "work", keep: "source" },
        { key: "PROJ-2", field: "status", keep: "source" },
      ],
    });
    fireEvent.click(screen.getByRole("button", { name: "Áp dụng 3 thay đổi" }));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    const pull = send.mock.calls.map(([command]) => command).find((command) => command.type === "apply_pull");
    expect(pull).toMatchObject({ type: "apply_pull", resolutions: [{ key: "PROJ-1", field: "name", keep: "source" }, { key: "PROJ-2" }, { key: "PROJ-2" }] });
  });

  it("can apply a choice that only keeps Sheafly even though no task changes", async () => {
    const { user, send, onClose } = renderDialog(projectWith(SOURCE), CONFLICTS);
    fakeBackend.pullChanges = {
      changes: CONFLICTS.changes,
      gone: [],
      problems: [],
      conflicts: [],
      bases_changed: true,
    };
    await user.click(fetchButton());
    await screen.findByRole("table", { name: "Xung đột" });
    await user.click(screen.getByRole("button", { name: "Giữ Sheafly cho tất cả" }));
    const apply = await screen.findByRole("button", { name: "Áp dụng 0 thay đổi" });
    await waitFor(() => expect(apply).toBeEnabled());
    fireEvent.click(apply);
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(send.mock.calls.some(([command]) => command.type === "apply_pull")).toBe(true);
  });

  it("lists other problems by issue", async () => {
    const withProblem: PullPreview = {
      ...PREVIEW,
      problems: [{ message: "Khóa này đã có ở dòng trên", row: 2, key: "PROJ-3", column: "key" }],
    };
    const { user } = renderDialog(projectWith(SOURCE), withProblem);
    await fetched(user);
    expect(within(screen.getByRole("list", { name: "Cần xử lý" })).getByText("PROJ-3: Khóa này đã có ở dòng trên")).toBeInTheDocument();
  });

  it("offers nothing to apply when nothing changed at the source", async () => {
    const same: PullPreview = { ...PREVIEW, changes: PREVIEW.changes.map((row) => ({ ...row, kind: "same" as const, fields: [] })), gone: [], warnings: [] };
    const { user } = renderDialog(projectWith(SOURCE), same);
    await user.click(fetchButton());
    expect(await screen.findByText("Không có gì thay đổi ở nguồn.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /^Áp dụng/ })).toBeDisabled();
  });

  it("goes back to the form when the pull is cancelled, and shows an error from the source", async () => {
    const { user } = renderDialog(projectWith(SOURCE), null);
    await user.click(fetchButton());
    await waitFor(() => expect(fetchButton()).toBeEnabled());
    expect(screen.queryByText(/^Thêm \d+ ·/)).not.toBeInTheDocument();

    fakeBackend.failNext("tracker_pull", "Site không nhận khóa API này");
    await user.click(fetchButton());
    expect(await screen.findByRole("alert")).toHaveTextContent("Site không nhận khóa API này");
    expect(fetchButton()).toBeEnabled();
  });

  it("stops a pull that is fetching when the dialog is closed", async () => {
    const release = (() => {
      const { user } = renderDialog(projectWith(SOURCE));
      const hold = fakeBackend.holdNext("tracker_pull");
      void user.click(fetchButton());
      return hold;
    })();
    await screen.findByRole("button", { name: "Dừng lấy" });
    fireEvent.keyDown(document, { key: "Escape" });
    await waitFor(() => expect(calls("tracker_cancel")).toHaveLength(1));
    release();
  });
});
