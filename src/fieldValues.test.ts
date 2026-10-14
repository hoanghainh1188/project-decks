import { afterEach, describe, expect, it } from "vitest";

import { distinctValues, formatFieldValue, parseFieldValue, valueKey, valueOf } from "./fieldValues";
import { setCurrentLang } from "./i18n";
import { makeTask } from "./test/fixtures";
import type { FieldDef } from "./types";

const TEXT: FieldDef = { id: 1, name: "Ghi chú", kind: { type: "text" } };
const NUMBER: FieldDef = { id: 2, name: "Điểm", kind: { type: "number" } };
const CHOICE: FieldDef = { id: 3, name: "Mảng", kind: { type: "choice", options: ["FE", "BE"] } };

afterEach(() => setCurrentLang("vi"));

describe("custom field values", () => {
  it("shows numbers with the language's decimal mark", () => {
    expect(formatFieldValue(7.5)).toBe("7,5");
    expect(formatFieldValue(1234.5)).toBe("1234,5");
    expect(formatFieldValue("A.B")).toBe("A.B");
    expect(formatFieldValue(undefined)).toBe("");
    setCurrentLang("en");
    expect(formatFieldValue(7.5)).toBe("7.5");
  });

  it("writes every digit and no exponent, so the text reads back exactly", () => {
    setCurrentLang("en");
    expect(formatFieldValue(1e21)).toBe("1000000000000000000000");
    expect(formatFieldValue(4e-7)).toBe("0.0000004");
    expect(parseFieldValue(NUMBER, formatFieldValue(4e-7))).toBe(4e-7);
  });

  it("reads only plain decimals in a number field", () => {
    for (const text of ["1.234,5", "0x10", "1e3", "1 2", "--1", "12,5,0"]) {
      expect(parseFieldValue(NUMBER, text), text).toBeUndefined();
    }
    expect(parseFieldValue(NUMBER, "-0,5")).toBe(-0.5);
    expect(parseFieldValue(NUMBER, ",5")).toBe(0.5);
    expect(parseFieldValue(TEXT, "x".repeat(256))).toBeUndefined();
  });

  it("reads typed text by the field's kind", () => {
    expect(parseFieldValue(NUMBER, " 12,5 ")).toBe(12.5);
    expect(parseFieldValue(NUMBER, "12.5")).toBe(12.5);
    expect(parseFieldValue(NUMBER, "mười")).toBeUndefined();
    expect(parseFieldValue(NUMBER, "  ")).toBeNull();
    expect(parseFieldValue(TEXT, "  có  ")).toBe("có");
    expect(parseFieldValue(CHOICE, "BE")).toBe("BE");
    expect(parseFieldValue(CHOICE, "QA")).toBeUndefined();
  });

  it("lists each value once, in the order a list shows them", () => {
    const tasks = [
      makeTask({ uid: 1, values: { 2: 10, 3: "BE", 1: "Bình" } }),
      makeTask({ uid: 2, values: { 2: 2, 3: "FE", 1: "An" } }),
      makeTask({ uid: 3, values: { 2: 10, 3: "BE" } }),
      makeTask({ uid: 4 }),
    ];
    expect(distinctValues(NUMBER, tasks)).toEqual([2, 10]);
    expect(distinctValues(CHOICE, tasks)).toEqual(["FE", "BE"]);
    expect(distinctValues(TEXT, tasks)).toEqual(["An", "Bình"]);
    expect(valueOf(tasks[0], NUMBER)).toBe(10);
    expect(valueKey(valueOf(tasks[3], NUMBER))).toBe("");
  });
});
