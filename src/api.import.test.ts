import { describe, expect, it } from "vitest";

import { applyCommand, checkImport, pickTable, readTableSheet } from "./api";
import { fakeBackend } from "./test/fakeBackend";

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
});
