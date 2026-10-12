import { act, render, screen, waitFor } from "@testing-library/react";
import { listen } from "@tauri-apps/api/event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { setCurrentLang } from "./i18n";
import { Root } from "./Root";
import { fakeBackend } from "./test/fakeBackend";

describe("Root", () => {
  beforeEach(() => {
    // inTauri() looks for this; the fake backend answers the commands.
    (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: "main" }, currentWebview: { label: "main" } },
    };
  });
  afterEach(() => {
    delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
    vi.restoreAllMocks();
    vi.mocked(listen).mockClear();
    setCurrentLang("vi");
  });

  it("uses the saved language and tells the backend to keep it", async () => {
    fakeBackend.language = "en";
    render(<Root />);
    expect(await screen.findByRole("button", { name: "Undo" })).toBeInTheDocument();
    expect(fakeBackend.calls).toContainEqual({ cmd: "set_language", args: { language: "en", remember: true } });
  });

  it("follows the system language without remembering it", async () => {
    vi.spyOn(navigator, "languages", "get").mockReturnValue(["vi-VN"]);
    render(<Root />);
    expect(await screen.findByRole("button", { name: "Hoàn tác" })).toBeInTheDocument();
    await waitFor(() =>
      expect(fakeBackend.calls).toContainEqual({ cmd: "set_language", args: { language: "vi", remember: false } }),
    );
    expect(fakeBackend.language).toBeNull();
  });

  it("falls back to the system language when reading the saved choice fails", async () => {
    vi.spyOn(navigator, "languages", "get").mockReturnValue(["en-GB"]);
    fakeBackend.failNext("get_language", "settings unreadable");
    render(<Root />);
    expect(await screen.findByRole("button", { name: "Undo" })).toBeInTheDocument();
  });

  it("switches from the language menu, remembers it, and works with a dialog open", async () => {
    vi.spyOn(navigator, "languages", "get").mockReturnValue(["vi-VN"]);
    render(<Root />);
    await screen.findByRole("button", { name: "Hoàn tác" });
    const menu = await menuHandler();
    // A dialog covers the window: the language switch is still allowed.
    const dialog = document.createElement("div");
    dialog.setAttribute("aria-modal", "true");
    document.body.append(dialog);
    await act(async () => menu("lang-en"));
    expect(await screen.findByRole("button", { name: "Undo" })).toBeInTheDocument();
    expect(fakeBackend.calls).toContainEqual({ cmd: "set_language", args: { language: "en", remember: true } });
    expect(fakeBackend.language).toBe("en");
    dialog.remove();
  });

  it("shows an error when the choice cannot be stored", async () => {
    vi.spyOn(navigator, "languages", "get").mockReturnValue(["vi-VN"]);
    render(<Root />);
    await screen.findByRole("button", { name: "Hoàn tác" });
    const menu = await menuHandler();
    // Startup loads clear the banner when they succeed; let them finish first.
    await waitFor(() => expect(fakeBackend.calls.some((call) => call.cmd === "check_autosave")).toBe(true));
    await act(async () => {});
    fakeBackend.failNext("set_language", "Không ghi được cài đặt");
    await act(async () => menu("lang-en"));
    expect(await screen.findByText("Không ghi được cài đặt")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Undo" })).toBeInTheDocument();
  });

  it("opens a file the app was launched with after the autosave offer", async () => {
    vi.spyOn(navigator, "languages", "get").mockReturnValue(["vi-VN"]);
    render(<Root />);
    await waitFor(() => expect(fakeBackend.calls.some((call) => call.cmd === "open_pending")).toBe(true));
    const order = fakeBackend.calls.map((call) => call.cmd);
    expect(order.indexOf("check_autosave")).toBeLessThan(order.indexOf("open_pending"));
  });

  it("opens a recent file from the menu and a file the system sends", async () => {
    vi.spyOn(navigator, "languages", "get").mockReturnValue(["vi-VN"]);
    render(<Root />);
    await screen.findByRole("button", { name: "Hoàn tác" });
    await waitFor(() => expect(fakeBackend.calls.some((call) => call.cmd === "open_pending")).toBe(true));
    const menu = await menuHandler();
    await act(async () => menu("recent:2"));
    await waitFor(() => expect(fakeBackend.calls).toContainEqual({ cmd: "open_recent", args: { index: 2 } }));
    const before = fakeBackend.calls.filter((call) => call.cmd === "open_pending").length;
    const openFile = await eventHandler("open-file");
    await act(async () => openFile({ payload: null }));
    await waitFor(() =>
      expect(fakeBackend.calls.filter((call) => call.cmd === "open_pending").length).toBe(before + 1),
    );
  });
});

/** The handler App registered for native menu clicks. */
async function menuHandler(): Promise<(id: string) => void> {
  let found: ((event: { payload: string }) => void) | undefined;
  await waitFor(() => {
    const call = vi.mocked(listen).mock.calls.find(([name]) => name === "menu-action");
    expect(call).toBeDefined();
    found = call?.[1] as unknown as (event: { payload: string }) => void;
  });
  return (id: string) => found?.({ payload: id });
}

/** The handler App registered for a backend event. */
async function eventHandler(name: string): Promise<(event: { payload: unknown }) => void> {
  let found: ((event: { payload: unknown }) => void) | undefined;
  await waitFor(() => {
    const call = vi.mocked(listen).mock.calls.find(([event]) => event === name);
    expect(call).toBeDefined();
    found = call?.[1] as unknown as (event: { payload: unknown }) => void;
  });
  return (event) => found?.(event);
}
