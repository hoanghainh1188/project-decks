import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { I18nProvider } from "../../i18n";
import { fakeBackend } from "../../test/fakeBackend";
import { makeProject, makeResource, MONDAY_8AM } from "../../test/fixtures";
import { LIGHT_CANVAS } from "../../theme";
import { LoadMapView } from "./LoadMapView";

const project = makeProject({
  start: MONDAY_8AM,
  resources: [makeResource({ uid: 10, name: "An" }), makeResource({ uid: 11, name: "Bình" })],
});

function recordingContext() {
  const fills: string[] = [];
  let fillStyle = "";
  const target = {
    get fillStyle() {
      return fillStyle;
    },
    set fillStyle(value: string) {
      fillStyle = value;
      fills.push(value);
    },
  };
  const ctx = new Proxy(target, {
    get: (object, key) => (key in object ? object[key as keyof typeof object] : () => {}),
    set: (object, key, value) => {
      (object as Record<string | symbol, unknown>)[key] = value;
      return true;
    },
  });
  return { ctx, fills };
}

function renderView(onPickResource = vi.fn(), lang: "vi" | "en" = "vi") {
  fakeBackend.loadMap = {
    buckets: ["2026-01-05", "2026-01-12"],
    rows: [
      { resource_uid: 10, cells: [{ work: 2400, capacity: 2400 }, { work: 3000, capacity: 2400 }] },
      { resource_uid: 11, cells: [{ work: 600, capacity: 2400 }, { work: 0, capacity: 0 }] },
    ],
  };
  const recorded = recordingContext();
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(recorded.ctx as never);
  render(
    <I18nProvider lang={lang}>
      <LoadMapView project={project} onPickResource={onPickResource} />
    </I18nProvider>,
  );
  return { onPickResource, recorded };
}

function lastLoadMap() {
  const calls = fakeBackend.calls.filter((call) => call.cmd === "load_map");
  return calls[calls.length - 1]?.args;
}

describe("LoadMapView", () => {
  it("asks for twelve weeks from the project start and lists people by name", async () => {
    renderView();
    await waitFor(() => expect(lastLoadMap()).toEqual({ from: "2026-01-05", to: "2026-03-29", bucket: "week" }));
    expect(await screen.findByRole("button", { name: "An" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Bình" })).toBeInTheDocument();
    expect(screen.getByRole("columnheader", { name: "05/01" })).toBeInTheDocument();
  });

  it("colours cells by how full they are", async () => {
    const { recorded } = renderView();
    await screen.findByRole("button", { name: "An" });
    await waitFor(() => expect(recorded.fills).toContain(LIGHT_CANVAS.loadOver));
    expect(recorded.fills).toContain(LIGHT_CANVAS.loadFull);
    expect(recorded.fills).toContain(LIGHT_CANVAS.loadLight);
    expect(recorded.fills).toContain(LIGHT_CANVAS.weekend);
  });

  it("explains a cell on hover and filters the tasks by a person on click", async () => {
    const { onPickResource } = renderView();
    await screen.findByRole("button", { name: "An" });
    const grid = screen.getByRole("img", { name: /Tải theo người/ });
    vi.spyOn(grid, "getBoundingClientRect").mockReturnValue({ left: 0, top: 0, right: 0, bottom: 0, width: 0, height: 0, x: 0, y: 0, toJSON: () => ({}) });
    // Second column, first row.
    fireEvent.mouseMove(grid, { clientX: 70, clientY: 10 });
    expect(grid).toHaveAttribute("title", "An · tuần 12/01/2026: 50 giờ việc / 40 giờ năng lực (125%)");
    fireEvent.click(grid, { clientX: 70, clientY: 10 });
    expect(onPickResource).toHaveBeenCalledWith(10);
    fireEvent.click(screen.getByRole("button", { name: "Bình" }));
    expect(onPickResource).toHaveBeenCalledWith(11);
  });

  it("switches to days and changes the span", async () => {
    renderView();
    await screen.findByRole("button", { name: "An" });
    fireEvent.click(screen.getByRole("radio", { name: "Ngày" }));
    await waitFor(() => expect(lastLoadMap()?.bucket).toBe("day"));
    const weeks = screen.getByLabelText("Số tuần");
    fireEvent.change(weeks, { target: { value: "" } });
    expect(weeks).toHaveValue(null);
    fireEvent.change(weeks, { target: { value: "4" } });
    await waitFor(() => expect(lastLoadMap()).toEqual({ from: "2026-01-05", to: "2026-02-01", bucket: "day" }));
  });

  it("speaks English", async () => {
    renderView(vi.fn(), "en");
    expect(await screen.findByRole("columnheader", { name: "01-05" })).toBeInTheDocument();
    expect(screen.getByRole("radio", { name: "Days" })).toBeInTheDocument();
  });
});
