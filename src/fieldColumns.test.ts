import { describe, expect, it } from "vitest";

import { toggleHidden, visibleFields } from "./fieldColumns";
import type { FieldDef } from "./types";

const fields: FieldDef[] = [
  { id: 1, name: "Khách hàng", kind: { type: "text" } },
  { id: 2, name: "Điểm", kind: { type: "number" } },
  { id: 3, name: "Nhóm", kind: { type: "text" } },
] as FieldDef[];

describe("field columns", () => {
  it("shows every field until one is hidden, keeping the order", () => {
    expect(visibleFields(fields, new Set())).toBe(fields);
    expect(visibleFields(fields, new Set([2])).map((field) => field.id)).toEqual([1, 3]);
  });

  it("toggles into a new set without touching the old one", () => {
    const empty: ReadonlySet<number> = new Set();
    const hidden = toggleHidden(empty, 2, false);
    expect([...hidden]).toEqual([2]);
    expect(empty.size).toBe(0);
    expect([...toggleHidden(hidden, 2, true)]).toEqual([]);
  });
});
