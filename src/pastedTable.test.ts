import { describe, expect, it } from "vitest";

import { parsePastedTable } from "./pastedTable";

describe("parsePastedTable", () => {
  it("reads tab-separated rows with their line numbers, leaving out blank rows", () => {
    const sheet = parsePastedTable("Tên\tThời lượng\r\nThiết kế\t3 ngày\r\n\t\r\nCode\t5\r\n");
    expect(sheet?.rows).toEqual([
      ["Tên", "Thời lượng"],
      ["Thiết kế", "3 ngày"],
      ["Code", "5"],
    ]);
    expect(sheet?.lines).toEqual([1, 2, 4]);
    expect(sheet?.sheets).toEqual([]);
  });

  it("keeps tabs, line breaks and quotes inside a quoted cell", () => {
    const sheet = parsePastedTable('A\t"dòng 1\ndòng 2"\nB\t"có ""nháy"" và\ttab"');
    expect(sheet?.rows).toEqual([
      ["A", "dòng 1\ndòng 2"],
      ["B", 'có "nháy" và\ttab'],
    ]);
    expect(sheet?.lines).toEqual([1, 3]);
  });

  it("pads short rows to the widest", () => {
    expect(parsePastedTable("a\tb\tc\nd")?.rows).toEqual([
      ["a", "b", "c"],
      ["d", "", ""],
    ]);
  });

  it("is not a table without a tab or a second line", () => {
    expect(parsePastedTable("Thiết kế")).toBeNull();
    expect(parsePastedTable("Thiết kế\n")).toBeNull();
    expect(parsePastedTable("\t\n\t")).toBeNull();
  });
});
