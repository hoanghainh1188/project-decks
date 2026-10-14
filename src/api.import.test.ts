import { describe, expect, it } from "vitest";

import { applyCommand, checkImport, pickTable, readTableSheet } from "./api";
import { fakeBackend } from "./test/fakeBackend";
import { makeProject, makeSnapshot, makeTask } from "./test/fixtures";

describe("import calls", () => {
  it("checks rows with the anchor and both tables", async () => {
    fakeBackend.importProblems = [{ table: "tasks", row: 2, column: { field: "Mảng" }, message: "Giá trị không hợp" }];
    const problems = await checkImport(1, [{ name: "An", kind: "person" }], [{ name: "Việc", after: [0] }]);
    expect(problems).toEqual(fakeBackend.importProblems);
    expect(fakeBackend.calls[fakeBackend.calls.length - 1]).toEqual({
      cmd: "check_import",
      args: { afterUid: 1, resources: [{ name: "An", kind: "person" }], tasks: [{ name: "Việc", after: [0] }] },
    });
  });

  it("returns null when the file dialog is cancelled and reads another sheet", async () => {
    expect(await pickTable()).toBeNull();
    fakeBackend.table = { sheets: ["Người", "Việc"], sheet: "Người", rows: [["Tên"]] };
    expect((await pickTable())?.sheets).toEqual(["Người", "Việc"]);
    expect((await readTableSheet("Việc")).sheet).toBe("Việc");
    await expect(readTableSheet("Khác")).rejects.toBe("Không có trang này trong tệp");
  });

  it("imports rows after the anchor in one command", async () => {
    const snapshot = await applyCommand({
      type: "import_rows",
      after_uid: null,
      resources: [{ name: "Bình", kind: "person" }],
      tasks: [{ name: "Nhập 1" }, { name: "Nhập 2", level: 1 }],
    });
    expect(snapshot.project.tasks.map((task) => task.name)).toEqual(["Nhập 1", "Nhập 2"]);
    expect(snapshot.project.resources.map((resource) => resource.name)).toEqual(["Bình"]);
  });

  it("puts imported rows after the anchor's subtree", async () => {
    const tasks = [
      makeTask({ uid: 1, name: "Nhóm", summary: true }),
      makeTask({ uid: 2, name: "Con", outline_level: 2, parent_uid: 1 }),
      makeTask({ uid: 3, name: "Sau" }),
    ];
    fakeBackend.reset(makeSnapshot(makeProject({ tasks, next_task_uid: 4 })));
    const snapshot = await applyCommand({ type: "import_rows", after_uid: 1, resources: [], tasks: [{ name: "Mới" }] });
    expect(snapshot.project.tasks.map((task) => task.name)).toEqual(["Nhóm", "Con", "Mới", "Sau"]);
  });
});
