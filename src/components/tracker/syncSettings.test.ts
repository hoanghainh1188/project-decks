import { describe, expect, it } from "vitest";

import { makeProject } from "../../test/fixtures";
import { DEFAULT_SYNC_SETTINGS, type SyncSettings } from "../../types";
import { defaultRow, fieldKeys, kindOf, toRows, toSettings } from "./syncSettings";

const project = makeProject({
  fields: [
    { id: 3, name: "Mảng", kind: { type: "text" } },
    { id: 5, name: "Điểm", kind: { type: "number" } },
  ],
});

const constants = { inProgressPercent: 50, startHour: 8, dueHour: 17 };

describe("syncSettings", () => {
  it("lists the built-in fields, the planned days, all custom fields and each custom field", () => {
    expect(fieldKeys(project)).toEqual([
      "name", "work", "start", "due", "status", "assignee", "planned_start", "planned_finish", "fields", "field:3", "field:5",
    ]);
  });

  it("offers a direction only where it means something", () => {
    expect(kindOf("start").directions).toEqual(["both", "pull", "push", "off"]);
    expect(kindOf("name").directions).toEqual(["pull", "off"]);
    expect(kindOf("planned_start").directions).toEqual(["push", "off"]);
    expect(kindOf("planned_start").writesTo).toBe(true);
    expect(kindOf("work").readsFrom && kindOf("start").readsFrom && kindOf("field:3").readsFrom).toBe(true);
    expect(kindOf("status").readsFrom).toBe(false);
    expect(kindOf("planned_finish").conflicts).toBe(false);
  });

  it("starts with every row at its default, and the defaults save as no policies at all", () => {
    const rows = toRows(DEFAULT_SYNC_SETTINGS, project);
    expect(rows.every((row) => JSON.stringify(row) === JSON.stringify(defaultRow(row.field)))).toBe(true);
    expect(toSettings(rows, constants)).toEqual(DEFAULT_SYNC_SETTINGS);
  });

  it("saves only the rows the user changed, with explicit values", () => {
    const rows = toRows(DEFAULT_SYNC_SETTINGS, project).map((row) => {
      if (row.field === "due") return { ...row, direction: "pull" as const, onConflict: "source" as const };
      if (row.field === "work") return { ...row, sourceField: " custom:Điểm " };
      if (row.field === "planned_start") return { ...row, pushField: "custom:Bắt đầu dự kiến" };
      if (row.field === "field:3") return { ...row, direction: "off" as const };
      return row;
    });
    const settings = toSettings(rows, { inProgressPercent: 30, startHour: 9, dueHour: 18 });
    expect(settings).toEqual({
      fields: [
        { field: "work", direction: "pull", on_conflict: "ask", source_field: "custom:Điểm", push_field: null },
        { field: "due", direction: "pull", on_conflict: "source", source_field: null, push_field: null },
        { field: "planned_start", direction: "push", on_conflict: "ask", source_field: null, push_field: "custom:Bắt đầu dự kiến" },
        { field: "field:3", direction: "off", on_conflict: "ask", source_field: null, push_field: null },
      ],
      in_progress_percent: 30,
      start_hour: 9,
      due_hour: 18,
    });
  });

  it("reads saved settings back into the same rows", () => {
    const saved: SyncSettings = {
      fields: [
        { field: "start", direction: "push", on_conflict: "sheafly", source_field: null, push_field: null },
        { field: "planned_finish", direction: "push", on_conflict: "ask", source_field: null, push_field: "custom:Kế hoạch" },
      ],
      in_progress_percent: 40,
      start_hour: 7,
      due_hour: 16,
    };
    const rows = toRows(saved, project);
    expect(rows.find((row) => row.field === "start")).toMatchObject({ direction: "push", onConflict: "sheafly" });
    expect(rows.find((row) => row.field === "planned_finish")).toMatchObject({ pushField: "custom:Kế hoạch" });
    expect(toSettings(rows, { inProgressPercent: 40, startHour: 7, dueHour: 16 }).fields).toEqual(
      saved.fields.map((policy) => ({ ...policy })),
    );
  });

  it("shows a both-ways policy on a pull-only row as pull, which is what it means", () => {
    const rows = toRows(
      { ...DEFAULT_SYNC_SETTINGS, fields: [{ field: "name", direction: "both", on_conflict: "source", source_field: null, push_field: null }] },
      project,
    );
    expect(rows[0]).toMatchObject({ field: "name", direction: "pull", onConflict: "source" });
  });

  it("drops a conflict choice on a row that cannot conflict", () => {
    const rows = toRows(DEFAULT_SYNC_SETTINGS, project).map((row) =>
      row.field === "planned_start" ? { ...row, onConflict: "source" as const, pushField: "custom:X" } : row,
    );
    const policy = toSettings(rows, constants).fields[0];
    expect(policy.on_conflict).toBe("ask");
  });
});
