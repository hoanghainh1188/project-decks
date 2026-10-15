import { useEffect, useMemo, useRef, useState, type MouseEvent } from "react";

import { errorMessage, loadMap } from "../../api";
import { formatDateKeyDisplay, formatHours, instantToDateKey } from "../../format";
import { getLang, useMessages } from "../../i18n";
import { canvasPalette, useResolvedTheme, type CanvasPalette } from "../../theme";
import type { LoadBucket, LoadCell, LoadMap, Project } from "../../types";
import { loadMapMessages } from "./LoadMapView.i18n";
import { loadTone, type LoadTone } from "./loadTone";

const ROW = 24;
// Wide enough for "05/01" or "01-05" at the header size, so day headers never touch.
const CELL = { day: 36, week: 60 } as const;
const DEFAULT_WEEKS = 12;
const MAX_WEEKS = 104;
const DAY_MS = 86_400_000;

/** Day and month as a column header fits it: "05/01" in Vietnamese, "01-05" in English. */
function shortDate(key: string): string {
  const [, month, day] = key.split("-");
  return getLang() === "en" ? `${month}-${day}` : `${day}/${month}`;
}

/** The whole date, for the tooltip. */
function longDate(key: string): string {
  return formatDateKeyDisplay(key);
}

function percent(cell: LoadCell): string {
  return cell.capacity > 0 ? `${Math.round((cell.work / cell.capacity) * 100)}%` : "";
}

function toneColour(tone: LoadTone, palette: CanvasPalette): string | null {
  switch (tone) {
    case "off":
      return palette.weekend;
    case "light":
      return palette.loadLight;
    case "full":
      return palette.loadFull;
    case "over":
      return palette.loadOver;
    default:
      return null;
  }
}

/**
 * Tải nguồn lực: a heatmap of each person's work against their capacity, by day or week,
 * from `load_map` (the engine counts both). People are buttons that show their tasks, as
 * is a click on any of their cells; hovering a cell gives its hours.
 */
export function LoadMapView({ project, onPickResource }: { project: Project; onPickResource: (uid: number) => void }) {
  const m = useMessages(loadMapMessages);
  const theme = useResolvedTheme();
  const [bucket, setBucket] = useState<LoadBucket>("week");
  const [from, setFrom] = useState(() => instantToDateKey(project.start));
  const [weeks, setWeeks] = useState(DEFAULT_WEEKS);
  // What is typed, so the field can be cleared and typed again; `weeks` follows when valid.
  const [weeksText, setWeeksText] = useState(String(DEFAULT_WEEKS));
  // The map with the bucket it was read for: until a new one arrives, the old one is
  // drawn and explained at its own width.
  const [loaded, setLoaded] = useState<{ map: LoadMap; bucket: LoadBucket } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [tip, setTip] = useState("");
  const canvas = useRef<HTMLCanvasElement>(null);

  const to = useMemo(() => {
    const start = Date.parse(`${from}T00:00:00Z`);
    return Number.isNaN(start) ? from : instantToDateKey(start + (weeks * 7 - 1) * DAY_MS);
  }, [from, weeks]);

  // Read again whenever the project changes: work and capacity follow every edit.
  useEffect(() => {
    let current = true;
    loadMap(from, to, bucket)
      .then((next) => {
        if (!current) return;
        setError(null);
        setLoaded({ map: next, bucket });
      })
      .catch((err) => current && setError(errorMessage(err)));
    return () => {
      current = false;
    };
  }, [from, to, bucket, project]);

  const names = useMemo(() => new Map(project.resources.map((item) => [item.uid, item.name])), [project.resources]);
  const map = loaded?.map ?? null;
  const shownBucket = loaded?.bucket ?? bucket;
  const width = CELL[shownBucket];

  useEffect(() => {
    const element = canvas.current;
    const ctx = element?.getContext("2d");
    if (!element || !ctx || !map) return;
    const palette = canvasPalette();
    const ratio = window.devicePixelRatio || 1;
    const cssWidth = map.buckets.length * width;
    const cssHeight = map.rows.length * ROW;
    element.width = cssWidth * ratio;
    element.height = cssHeight * ratio;
    element.style.width = `${cssWidth}px`;
    element.style.height = `${cssHeight}px`;
    ctx.setTransform(ratio, 0, 0, ratio, 0, 0);
    ctx.clearRect(0, 0, cssWidth, cssHeight);
    map.rows.forEach((row, y) => {
      row.cells.forEach((cell, x) => {
        const colour = toneColour(loadTone(cell), palette);
        if (!colour) return;
        ctx.fillStyle = colour;
        ctx.fillRect(x * width + 1, y * ROW + 1, width - 2, ROW - 2);
      });
    });
    ctx.fillStyle = palette.grid;
    for (let y = 1; y < map.rows.length; y++) ctx.fillRect(0, y * ROW, cssWidth, 1);
  }, [map, width, theme]);

  function cellAt(event: MouseEvent<HTMLCanvasElement>) {
    if (!map) return null;
    const bounds = event.currentTarget.getBoundingClientRect();
    const x = Math.floor((event.clientX - bounds.left) / width);
    const y = Math.floor((event.clientY - bounds.top) / ROW);
    const row = map.rows[y];
    const cell = row?.cells[x];
    return row && cell ? { row, cell, bucket: map.buckets[x] } : null;
  }

  function hover(event: MouseEvent<HTMLCanvasElement>) {
    const found = cellAt(event);
    if (!found) return setTip("");
    const name = names.get(found.row.resource_uid) ?? String(found.row.resource_uid);
    const when = shownBucket === "week" ? m.weekOf(longDate(found.bucket)) : longDate(found.bucket);
    const hours = (minutes: number) => formatHours(minutes / 60) || "0";
    setTip(m.tip(name, when, hours(found.cell.work), hours(found.cell.capacity), percent(found.cell)));
  }

  return (
    <div className="load-view">
      <div className="load-controls">
        <fieldset className="import-target">
          <legend>{m.bucket}</legend>
          {(["day", "week"] as const).map((item) => (
            <label key={item}>
              <input type="radio" name="load-bucket" checked={bucket === item} onChange={() => setBucket(item)} /> {m[item]}
            </label>
          ))}
        </fieldset>
        <label>
          {m.from} <input type="date" value={from} onChange={(event) => event.target.value && setFrom(event.target.value)} />
        </label>
        <label>
          {m.weeks}{" "}
          <input
            type="number"
            min={1}
            max={MAX_WEEKS}
            value={weeksText}
            onChange={(event) => {
              setWeeksText(event.target.value);
              const value = Number(event.target.value);
              if (Number.isInteger(value) && value >= 1 && value <= MAX_WEEKS) setWeeks(value);
            }}
            onBlur={() => setWeeksText(String(weeks))}
          />
        </label>
        <Legend />
      </div>
      {error && (
        <p className="bad" role="alert">
          {error}
        </p>
      )}
      {project.resources.length === 0 ? (
        <p className="meta">{m.nobody}</p>
      ) : !map ? (
        <p className="meta">{m.loading}</p>
      ) : (
        <div className="load-scroll">
          <div className="load-grid" style={{ gridTemplateColumns: `max-content ${map.buckets.length * width}px` }}>
            <div className="load-corner">{m.person}</div>
            <div className="load-head" role="row">
              {map.buckets.map((key) => (
                <span key={key} role="columnheader" style={{ width }}>
                  {shortDate(key)}
                </span>
              ))}
            </div>
            <div className="load-names">
              {map.rows.map((row) => {
                const name = names.get(row.resource_uid) ?? String(row.resource_uid);
                return (
                  <button key={row.resource_uid} type="button" title={m.pick(name)} onClick={() => onPickResource(row.resource_uid)}>
                    {name}
                  </button>
                );
              })}
            </div>
            <canvas
              ref={canvas}
              role="img"
              aria-label={m.grid(map.rows.length, map.buckets.length)}
              title={tip}
              onMouseMove={hover}
              onMouseLeave={() => setTip("")}
              onClick={(event) => {
                const found = cellAt(event);
                if (found) onPickResource(found.row.resource_uid);
              }}
            />
          </div>
        </div>
      )}
    </div>
  );
}

function Legend() {
  const m = useMessages(loadMapMessages);
  const tones: LoadTone[] = ["free", "light", "full", "over", "off"];
  return (
    <ul className="load-legend" aria-label={m.legend}>
      {tones.map((tone) => (
        <li key={tone}>
          <span className={`load-swatch ${tone}`} aria-hidden="true" />
          {m[tone]}
        </li>
      ))}
    </ul>
  );
}
