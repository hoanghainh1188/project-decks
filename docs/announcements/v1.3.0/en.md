---
title: "Sheafly 1.3.0: plan against your team's real capacity"
tags: [Sheafly, project management, Gantt, open source, Rust, Tauri]
---

# Sheafly 1.3.0: plan against your team's real capacity

**Sheafly** is a desktop project planner for macOS, Windows and Linux that works the way Microsoft Project does: you describe the tasks, links, working calendars and resources, and Sheafly works out the dates, the critical path, costs, overallocations and earned value. It is free and open source (MIT license), with an English and a Vietnamese interface.

The new **1.3.0** release is built around a question most planners run into: *does this plan fit the people who will actually do the work?*

![Sheafly with the task sheet, the Gantt chart and the metrics panel](../../images/screenshot-en.jpg)

Download: [github.com/hoanghainh1188/sheafly/releases](https://github.com/hoanghainh1188/sheafly/releases)

---

## What Sheafly does

### Scheduling like Microsoft Project

- FS, SS, FF and SF links with lag; critical path (CPM) and slack.
- Date constraints (SNET, MSO, MFO…), deadlines, manually scheduled tasks, split tasks and recurring tasks.
- The three task types, Fixed Duration, Fixed Units and Fixed Work, plus effort-driven scheduling: change one of duration, units or work and Sheafly recalculates the other two.
- Working calendars by the hour, with holidays, exceptions and work weeks with their own hours for a span of dates. A calendar can be based on another one, so a project holiday reaches every resource calendar.

### Tracking progress and cost

- Baselines 0–10, and a Tracking Gantt to compare plan against actuals.
- Actual start and finish dates move the schedule exactly as in Project; "Reschedule uncompleted work" moves the remaining work past the status date.
- Labour and fixed costs; actual costs can be entered by hand on tasks or on individual assignments.
- **EVM** (PV, EV, AC, SPI, CPI, EAC…), **burn-down** and **S-curve** reports.

### Many ways to look at a plan

Gantt, Tracking Gantt, Resource Sheet, Task Usage, Network Diagram and, new in this release, **Resource Load**. Filter, group and highlight by any field; export the Gantt to PDF.

### Your data stays safe

- A `.sheafly` file keeps the whole undo history: reopen it tomorrow and Undo still works.
- Saves are atomic: a power cut in the middle of a save never leaves a broken file.
- Autosave every 30 seconds, with recovery after a crash.
- A partly damaged file still opens, at the last state Sheafly can rebuild, with a report of what was lost.

### Working with other tools

- Import and export **MSPDI (XML)** to work alongside Microsoft Project: deadlines, progress, actual work, costs, work weeks, resource availability and custom fields.
- After an import, Sheafly lists what it read differently or left out, such as tasks that move from the file's dates.
- **New:** import tasks or people from **CSV or Excel**.

---

## What's new in 1.3.0

### Planning by capacity

This is the heart of the release. Instead of laying out the work and only then finding someone booked for 16 hours a day, tell Sheafly who can do what and when they are free, and let it make the suggestions.

- **Availability by date.** Each person can have periods with their own capacity: leave, part time, a later start on the project. Enter units or hours per day ("4 hrs/day").
- **Skills and assignment rules.** Name skills, give them to people, and write rules that pick a person, or everyone with a skill, from a task's custom fields.
- **Assign by rules.** Sheafly suggests people for the unassigned tasks, spread over everyone with the skill. **Before you apply it**, you see which tasks move, which deadlines are missed and who is still overallocated. Once applied, a single Undo takes it back.
- **Urgent tasks.** Tick "Interrupts" and leveling lets the task go ahead of lower-ranked work in progress; that work is split around it, and the Gantt draws the gaps as dashed boxes.
- **Resource Load view.** Each person's work against their capacity, by day or by week. Click a person to see what they are working on.
- **Outside the span.** Filter or highlight the tasks that finish after a date, after a milestone, or after their own deadline: a quick way to see what will not make the sprint.

### Leveling in milliseconds

Leveling 5,000 tasks on 500 people now takes about **40 ms**, down from **46 seconds**. The order tasks are placed in (start, priority, slack, deadline, duration, ID or a custom field, each ascending or descending) is set in Project Information → Leveling.

### Custom fields

Text, number and choice fields on tasks: sheet columns, an inspector group, filters, grouping and fill-down. They are exported to Microsoft Project as extended attributes.

### And more

- A dark theme (follow the system, or stay light or dark).
- Copy and paste whole task subtrees, with their assignments and links, between projects; drag individual segments of a split task on the Gantt.
- Project Information has a Statistics tab, and a Working Time dialog edits every calendar.
- macOS: no more "damaged app" message; the system now offers **Open Anyway**.

The full list is in the [1.3.0 release notes](https://github.com/hoanghainh1188/sheafly/releases/tag/v1.3.0).

---

## Why Sheafly

- **It runs on your machine.** No account, no server; your project lives in your file.
- **One engine does the maths.** Every number (dates, slack, costs, EVM) comes from an engine written in Rust; the interface only displays it. The same data always gives the same result.
- **Fast on large projects.** Every edit reschedules the whole project, and thousands of tasks still respond instantly.
- **Two languages, done properly.** English uses Microsoft Project's terms and ISO dates (`2026-01-05`); Vietnamese uses `05/01/2026`, `3.480.000` and `7,5 ngày`.

---

## What's next

These are the directions we are heading in. They are plans, not dates, and the order may change with your feedback.

### Schedule risk analysis with Monte Carlo

A single finish date is often misleading. You will be able to give each task a range estimate (low–high), and Sheafly will run thousands of simulations on its own scheduler and leveler, then tell you the **P50 and P90** finish dates: "a 50% chance of finishing by X, 90% by Y". Tasks whose deadlines are at risk get flagged. The results are computed figures that never change your plan, and the same data always gives the same numbers.

The leveling speed-up in 1.3.0 is the groundwork for this: each simulation run has to be fast enough for a few thousand runs to finish in under a minute.

### Recommendations from AI

Connect to AI models that read your plan and make recommendations: which tasks are the bottleneck, who could take over some work, which links look missing, which risks deserve attention. The principle stays the same as with assign by rules: the AI **suggests**, you preview and decide, and every change can be undone. The numbers still come from the engine, not from the AI's guesses.

### Connecting to JIRA, Backlog and Asana

Sync work between Sheafly and the task trackers your team already uses: pull in issues, assignees, estimates and status to plan the whole project and track progress, and push the forecast dates back. The team keeps working day to day in the tools it knows, while the project manager sees the schedule, capacity and risk in Sheafly.

### Other plans

- Signed and notarized installers, so installing no longer needs the "Open Anyway" step.
- Faster task sheet and Gantt with tens of thousands of rows.
- More interface languages (Japanese, for example).

---

## Try it

| System | File |
|---|---|
| macOS (Apple Silicon and Intel) | `Sheafly_1.3.0_universal.dmg` |
| Windows | `.msi` or `-setup.exe` |
| Linux | `.AppImage`, `.deb` or `.rpm` |

The installers are not signed with a developer certificate yet:

- **macOS:** try to open Sheafly once, then go to **System Settings → Privacy & Security** and click **Open Anyway**.
- **Windows:** if SmartScreen asks, choose **More info → Run anyway**.

Sheafly 1.3.0 opens every project from 1.0.0 to 1.2.0. A project saved with 1.3.0 cannot be opened in an older version.

Feedback, bug reports and feature ideas: [GitHub Issues](https://github.com/hoanghainh1188/sheafly/issues). If your team uses JIRA, Backlog or Asana, we would especially like to hear what you would want synced.
