# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

ProjectDesk is a Microsoft Project–style scheduling app for macOS built with Tauri 2, React 19, and Rust. The UI and user-facing strings are in Vietnamese, and so are the README and the plan doc.

## Commands

```bash
pnpm install
pnpm tauri dev                          # run the desktop app (Vite on :1420 + Rust shell)
pnpm build                              # tsc + vite build (frontend only)
pnpm exec tsc --noEmit                  # typecheck
pnpm test                               # frontend tests (Vitest, jsdom)
pnpm test src/viewRows.test.ts          # a single frontend test file (add -t "<name>" for one case)
cargo test -p projectdesk-engine        # engine tests
cargo test -p projectdesk-engine <name> # engine tests whose name matches the filter
```

CI (`.github/workflows/ci.yml`) runs the engine tests, the typecheck, and `pnpm test`. `.github/workflows/build.yml` builds unsigned installers for macOS, Windows, and Linux. It runs on `v*` tags, which also create a draft release, on manual dispatch, and on PRs that touch the Rust code, `package.json`, or the lockfiles. The real UI only works inside Tauri: `inTauri()` guards every `invoke`, so a plain browser at `localhost:1420` can't load project state.

## Architecture

Cargo workspace with two members: `crates/engine` (pure Rust, no Tauri dependency) and `src-tauri` (thin IPC shell). The React app lives in `src/`.

**The engine is the source of truth.** The UI never computes dates, slack, the critical path, costs, EVM, or burn-down numbers. It sends a `Command` and renders the `Snapshot` it gets back.

- **Time model** (`crates/engine/src/lib.rs`, `timeutil.rs`): instants are Unix milliseconds holding *project wall-clock time* formatted as UTC (no timezone conversion). Durations, lags, and work are **working minutes**. `Calendar` converts between the two.
- **Command log and undo** (`command.rs`, `document.rs`): every edit is a variant of the serde-tagged `Command` enum (`#[serde(tag = "type", rename_all = "snake_case")]`). `Document::apply` runs `apply_one` on a clone, then `schedule()`, and commits only if both succeed. Undo and redo **replay** `base` plus `commands[..cursor]` instead of reversing anything, so every command must be deterministic and replayable from the base project.
- **Scheduling** (`schedule.rs`): `schedule()` runs after every command. It normalizes the outline/WBS, detects cycles, runs forward/backward CPM passes (with a few passes for summary finish constraints), then rolls up summaries and publishes derived fields: free slack, costs and usage (`work.rs`), overallocations (`level.rs`), predecessor text, and the late flag. Derived fields live on the model structs and are overwritten on every schedule run.
- **Persistence** (`store.rs`): a `.pdproj` file is SQLite with `meta` (version, cursor, base project JSON), `command_log` (serialized commands), and `burndown`. On load the engine replays the log. Burn-down snapshots sit outside the command log on purpose, so undo doesn't remove them.
- **MSPDI** (`mspdi.rs`): XML import/export for Microsoft Project interop. There is no `.mpp` reader. Importing replaces the document (`replace_project`), which clears history.
- **Tauri layer** (`src-tauri/src/lib.rs`): holds `Mutex<Document>`, exposes the commands wrapped by `src/api.ts`, and builds the native macOS menu, which sends `menu-action` events to the frontend. File dialogs must use the plugin's non-blocking `pick_file`/`save_file` through `await_pick`/`await_save`. `blocking_pick_file` deadlocks on the macOS main thread. The `dialog_open` flag stops two dialogs from opening at once.
- **Frontend** (`src/`): `types.ts` is a **hand-written mirror** of the Rust serde types, including the `Command` union. When you add or change a `Command` variant or a model field in Rust, update `types.ts` to match. `App.tsx` owns snapshot state and menu handling. `viewRows.ts` does filter, group, and highlight on the client side as a view-only transform. `ganttLayout.ts` and `GanttChart.tsx` render the Gantt. `ganttPdf.ts` exports it to PDF with jsPDF and the embedded Be Vietnam Pro font (`src/fonts/`), which provides the Vietnamese glyphs.

## Tests

- **Engine:** integration tests in `crates/engine/tests/`. `engine.rs` covers the core; `limits.rs` covers the follow-ups to phases 9–10 (recurrence patterns, series units, older files, time-phased AC, S-curve). Fixtures use `at(day_offset, hour, minute)`, based on Monday 2026-01-05. The default calendar is Mon–Fri, 08:00–12:00 and 13:00–17:00, so one working day is 480 minutes.
- **Frontend:** `src/**/*.test.{ts,tsx}` with Vitest and Testing Library. `src/test/setup.ts` mocks `@tauri-apps/api/core` and `@tauri-apps/api/event`, so `invoke` goes to `src/test/fakeBackend.ts`. That fake records calls and applies a few commands in memory; it does not run the engine. Use `fakeBackend.reset(...)` and `failNext(...)` with the builders in `src/test/fixtures.ts`. If a component sends a command the fake doesn't handle yet, extend the fake.

## Plan and roadmap

`.cursor/plans/projectdesk_macos_da6c3154.plan.md` is the project plan, written in Vietnamese. It records the decisions behind each phase (1–10 are done), known limitations, and out-of-scope items such as task splitting, network diagrams, and multi-user. Keep it in sync when a phase or fix lands. Past PRs have updated it.
