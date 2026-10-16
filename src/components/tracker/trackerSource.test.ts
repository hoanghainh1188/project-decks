import { describe, expect, it } from "vitest";

import { DEFAULT_SYNC_SETTINGS, type Source } from "../../types";
import { complete, formFor, needsAccount, sameSource, sourceOf, TRACKER_KINDS, unmatchedEmails } from "./trackerSource";

const JIRA: Source = {
  kind: "jira",
  site: "team.atlassian.net",
  project_key: "PROJ",
  filter: "labels = plan",
  hours_per_point: 3,
  status_map: [],
  sync: { ...DEFAULT_SYNC_SETTINGS, in_progress_percent: 30 },
};

describe("trackerSource", () => {
  it("offers Backlog and Jira, and only Jira signs in with an account", () => {
    expect(TRACKER_KINDS).toEqual(["backlog", "jira"]);
    expect(needsAccount("jira")).toBe(true);
    expect(needsAccount("backlog")).toBe(false);
  });

  it("starts a form on Backlog with no account, or on the tracker the source has", () => {
    expect(formFor(null)).toMatchObject({ kind: "backlog", account: "", site: "" });
    expect(formFor(JIRA)).toMatchObject({ kind: "jira", account: "", site: "team.atlassian.net", filter: "labels = plan" });
  });

  it("builds the source with the form's tracker, keeping the project's settings and points factor", () => {
    const form = { ...formFor(JIRA), site: " Team.Atlassian.net ", projectKey: " PROJ ", account: "an@example.com" };
    const source = sourceOf(form, JIRA);
    expect(source).toEqual(JIRA);
    // The account is a credential: it is not part of the source.
    expect(JSON.stringify(source)).not.toContain("an@example.com");
    expect(sourceOf({ ...form, kind: "backlog" }, JIRA).kind).toBe("backlog");
  });

  it("compares a saved source with the form's by value, tracker included", () => {
    expect(sameSource(JIRA, sourceOf(formFor(JIRA), JIRA))).toBe(true);
    expect(sameSource(JIRA, { ...JIRA, kind: "backlog" })).toBe(false);
    expect(sameSource(null, JIRA)).toBe(false);
    expect(complete(JIRA)).toBe(true);
    expect(complete({ ...JIRA, project_key: "" })).toBe(false);
  });

  it("lists each email whose person the project lacks once", () => {
    const rows = [{ assignee_email: "a@x.com" }, { assignee_email: "a@x.com" }, { assignee_email: null }, { assignee_email: "b@x.com" }];
    const problems = [0, 1, 2, 3].map((row) => ({ message: "", row, key: "", column: "assignee" as const }));
    expect(unmatchedEmails(problems, rows)).toEqual(["a@x.com", "b@x.com"]);
  });
});
