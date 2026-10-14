import { afterEach, describe, expect, it } from "vitest";

import { setCurrentLang } from "./i18n";
import { guessColumns, type ImportSetup } from "./importColumns";
import { convertTable, engineIssues } from "./importConvert";
import { makeProject } from "./test/fixtures";
import type { FieldDef, ImportSheet } from "./types";

const FIELDS: FieldDef[] = [
  { id: 1, name: "Mảng", kind: { type: "choice", options: ["BE", "FE"] } },
  { id: 2, name: "Điểm", kind: { type: "number" } },
];
const project = makeProject({ fields: FIELDS });

function sheet(rows: string[][]): ImportSheet {
  return { sheets: [], sheet: null, rows, lines: rows.map((_, index) => index + 1) };
}

function tasksSetup(columns: string[]): ImportSetup {
  return { target: "tasks", headerRow: 0, columns, bareWork: "hours", bareDuration: "days" };
}

afterEach(() => setCurrentLang("vi"));

describe("guessColumns", () => {
  it("knows Vietnamese and English headers, accents and case aside", () => {
    expect(
      guessColumns(["Tên công việc", "EFFORT", "Ưu tiên", "Hạn chót", "Nguoi lam", "Chờ", "mảng", "Ghi chú"], "tasks", FIELDS),
    ).toEqual(["name", "work", "priority", "deadline", "assignees", "after", "field:1", "skip"]);
    expect(guessColumns(["Task name", "Duration", "Start", "Assignee", "Units", "Level", "Points"], "tasks", FIELDS)).toEqual([
      "name",
      "duration",
      "start_no_earlier",
      "assignees",
      "units",
      "level",
      "skip",
    ]);
  });

  it("gives each kind to the first column only", () => {
    expect(guessColumns(["Tên", "Name"], "tasks", [])).toEqual(["name", "skip"]);
  });

  it("guesses resource columns", () => {
    expect(guessColumns(["Họ tên", "Loại", "Đơn vị tối đa", "Đơn giá", "Kỹ năng"], "resources", [])).toEqual([
      "name",
      "kind",
      "max_units",
      "rate",
      "skills",
    ]);
  });
});

describe("convertTable for tasks", () => {
  const columns = ["name", "level", "work", "priority", "deadline", "start_no_earlier", "assignees", "units", "after", "field:1", "field:2"];
  const header = ["Tên", "Cấp", "Effort", "Ưu tiên", "Hạn", "Bắt đầu", "Người", "Đơn vị", "Chờ", "Mảng", "Điểm"];

  it("converts each cell the way the rest of the UI reads it", () => {
    const converted = convertTable(
      sheet([
        header,
        ["Nhóm", "1", "", "", "", "", "", "", "", "", ""],
        ["Thiết kế", "2", "16", "900", "09/01/2026", "2026-01-07 13:00", "An; Bình", "50%", "", "BE", "3,5"],
        ["", "", "", "", "", "", "", "", "", "", ""],
        ["Làm", "2", "2 ngày", "", "2026-01-12 00:00", "", "an", "", "3", "", ""],
      ]),
      tasksSetup(columns),
      project,
    );
    expect(converted.issues).toEqual([]);
    expect(converted.lines).toEqual([2, 3, 5]);
    expect(converted.tasks).toEqual([
      { name: "Nhóm", level: 0, values: [], assignees: [], after: [] },
      {
        name: "Thiết kế",
        level: 1,
        work: 16 * 60,
        priority: 900,
        // A deadline without a time is the end of that working day.
        deadline: Date.UTC(2026, 0, 9, 17, 0),
        start_no_earlier: Date.UTC(2026, 0, 7, 13, 0),
        values: [
          ["Mảng", "BE"],
          ["Điểm", 3.5],
        ],
        assignees: [
          { name: "An", units: 0.5 },
          { name: "Bình", units: 0.5 },
        ],
        after: [],
      },
      {
        name: "Làm",
        level: 1,
        work: 2 * 480,
        deadline: Date.UTC(2026, 0, 12, 17, 0),
        values: [],
        assignees: [{ name: "an", units: 1 }],
        // Line 3 of the file is the second task.
        after: [1],
      },
    ]);
  });

  it("reads bare effort and duration numbers in the unit chosen", () => {
    const converted = convertTable(sheet([["Tên", "Effort", "Thời lượng"], ["A", "2", ""], ["B", "", "1,5"]]), {
      ...tasksSetup(["name", "work", "duration"]),
      bareWork: "days",
      bareDuration: "hours",
    }, project);
    expect(converted.tasks.map((task) => [task.work, task.duration])).toEqual([
      [960, undefined],
      [undefined, 90],
    ]);
  });

  it("puts text in Unicode composed form, so names typed on macOS match", () => {
    const decomposed = "Nguyễn".normalize("NFD");
    const converted = convertTable(sheet([["Tên", "Người"], [`Việc ${decomposed}`, decomposed]]), tasksSetup(["name", "assignees"]), project);
    expect(converted.tasks[0].name).toBe("Việc Nguyễn");
    expect(converted.tasks[0].assignees).toEqual([{ name: "Nguyễn", units: 1 }]);
  });

  it("lists cells it cannot read with their line and column", () => {
    const converted = convertTable(
      sheet([
        header,
        ["A", "0", "nhiều", "cao", "mai", "", "", "x", "99", "QA", "ba"],
      ]),
      tasksSetup(columns),
      project,
    );
    expect(converted.issues.map((issue) => [issue.line, issue.column])).toEqual([
      [2, 1],
      [2, 2],
      [2, 3],
      [2, 4],
      [2, 7],
      [2, 8],
      [2, 9],
      [2, 10],
    ]);
    expect(converted.issues.every((issue) => issue.message.length > 0)).toBe(true);
  });

  it("needs a name column", () => {
    const converted = convertTable(sheet([["Effort"], ["1"]]), tasksSetup(["work"]), project);
    expect(converted.issues).toEqual([{ line: null, column: null, message: "Chọn cột Tên" }]);
  });

  it("starts after the header row chosen", () => {
    const rows = [["Báo cáo tháng"], ["Tên", "Effort"], ["A", "1"]];
    const converted = convertTable(sheet(rows), { ...tasksSetup(["name", "work"]), headerRow: 1 }, project);
    expect(converted.tasks.map((task) => task.name)).toEqual(["A"]);
    expect(converted.lines).toEqual([3]);
  });
});

describe("convertTable for resources", () => {
  it("reads kinds, units, rates and skills", () => {
    const converted = convertTable(
      sheet([
        ["Tên", "Loại", "Tối đa", "Đơn giá", "Kỹ năng"],
        ["An", "Người", "50%", "200.000", "Backend; SQL"],
        ["Máy in", "machine", "2", "", ""],
        ["Chi", "robot", "x", "-1", ""],
        ["Dũng", "", "", "12.5", ""],
      ]),
      { target: "resources", headerRow: 0, columns: ["name", "kind", "max_units", "rate", "skills"], bareWork: "hours", bareDuration: "days" },
      project,
    );
    expect(converted.resources[3].standard_rate).toBe(12.5);
    expect(converted.resources.slice(0, 2)).toEqual([
      { name: "An", kind: "person", max_units: 0.5, standard_rate: 200000, skills: ["Backend", "SQL"] },
      { name: "Máy in", kind: "machine", max_units: 2, skills: [] },
    ]);
    expect(converted.issues.map((issue) => [issue.line, issue.column])).toEqual([
      [4, 1],
      [4, 2],
      [4, 3],
    ]);
  });
});

describe("engineIssues", () => {
  it("shows the engine's problems at the file line and column they belong to", () => {
    const setup = tasksSetup(["name", "assignees", "units", "field:1"]);
    const converted = convertTable(sheet([["Tên", "Người", "Đơn vị", "Mảng"], ["A", "Lạ", "", ""], ["B", "", "", "BE"]]), setup, project);
    const issues = engineIssues(
      [
        { table: "tasks", row: 0, column: "assignees", message: "Dự án không có người tên này" },
        { table: "tasks", row: 1, column: { field: "mảng" }, message: "Giá trị không hợp" },
        { table: "tasks", row: null, column: "row", message: "Phụ thuộc tạo thành vòng lặp" },
      ],
      converted,
      setup,
      project.fields,
    );
    expect(issues).toEqual([
      { line: 2, column: 1, message: "Dự án không có người tên này" },
      { line: 3, column: 3, message: "Giá trị không hợp" },
      { line: null, column: null, message: "Phụ thuộc tạo thành vòng lặp" },
    ]);
  });
});
