import { useSyncExternalStore } from "react";

/** What the user chose in View → Appearance. */
export type ThemeChoice = "system" | "light" | "dark";
/** What is on screen: a "system" choice resolved against the system setting. */
export type ResolvedTheme = "light" | "dark";

export const THEME_CHOICES: readonly ThemeChoice[] = ["system", "light", "dark"];

export function isThemeChoice(value: unknown): value is ThemeChoice {
  return typeof value === "string" && (THEME_CHOICES as readonly string[]).includes(value);
}

const DARK_QUERY = "(prefers-color-scheme: dark)";

let choice: ThemeChoice = "system";
let resolved: ResolvedTheme = "light";
const listeners = new Set<() => void>();
let media: MediaQueryList | null = null;

function systemDark(): boolean {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") return false;
  media ??= window.matchMedia(DARK_QUERY);
  return media.matches;
}

function update() {
  const next: ResolvedTheme = choice === "dark" || (choice === "system" && systemDark()) ? "dark" : "light";
  if (typeof document !== "undefined") document.documentElement.dataset.theme = next;
  if (next === resolved) return;
  resolved = next;
  listeners.forEach((listener) => listener());
}

let watching = false;
/** Follow the system setting while the choice is "system" (and ignore it otherwise). */
function watchSystem() {
  if (watching) return;
  systemDark();
  if (!media) return;
  watching = true;
  media.addEventListener("change", update);
}

/** Show `next` now: sets `data-theme` on <html>, which switches the CSS tokens. */
export function applyTheme(next: ThemeChoice) {
  choice = next;
  watchSystem();
  update();
}

export function themeChoice(): ThemeChoice {
  return choice;
}

export function resolvedTheme(): ResolvedTheme {
  return resolved;
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** The theme on screen; components that draw on a canvas repaint when it changes. */
export function useResolvedTheme(): ResolvedTheme {
  return useSyncExternalStore(subscribe, resolvedTheme, resolvedTheme);
}

/**
 * Canvas colours, by name, with the CSS token each one reads and its light value. The
 * light values are also used where the tokens are not loaded (tests); theme.test.ts
 * checks they match tokens.css.
 */
export const CANVAS_TOKENS = {
  weekend: ["--weekend", "#f3f3f5"],
  grid: ["--grid", "#ececf0"],
  gridRow: ["--grid-row", "#f0f0f2"],
  group: ["--surface-group", "#ececf1"],
  groupRule: ["--border-subtle", "#e4e4e8"],
  text: ["--text", "#1d1d1f"],
  label: ["--text-secondary", "#3a3a3c"],
  labelHalo: ["--label-halo", "rgba(255, 255, 255, 0.9)"],
  highlight: ["--highlight-wash", "rgba(255, 214, 64, 0.38)"],
  selected: ["--selected-wash", "rgba(10, 132, 255, 0.1)"],
  task: ["--bar-task", "#2f6fad"],
  critical: ["--bar-critical", "#c23b3b"],
  manual: ["--bar-manual", "#2a9d8f"],
  summary: ["--bar-summary", "#1d1d1f"],
  baseline: ["--bar-baseline", "#c5c5cb"],
  progress: ["--bar-progress", "rgba(0, 0, 0, 0.28)"],
  knob: ["--bar-knob", "#ffffff"],
  levelingGap: ["--bar-leveling-gap", "#d97706"],
  link: ["--link", "#6d6d72"],
  accent: ["--accent", "#0a84ff"],
} as const satisfies Record<string, readonly [string, string]>;

export type CanvasPalette = { [K in keyof typeof CANVAS_TOKENS]: string };

/** The light palette, as PDF export and tests use it. */
export const LIGHT_CANVAS: CanvasPalette = Object.fromEntries(
  Object.entries(CANVAS_TOKENS).map(([key, [, light]]) => [key, light]),
) as CanvasPalette;

/** The canvas colours of the theme on screen, read from the CSS tokens on <html>. */
export function canvasPalette(): CanvasPalette {
  if (typeof document === "undefined") return LIGHT_CANVAS;
  const style = getComputedStyle(document.documentElement);
  return Object.fromEntries(
    Object.entries(CANVAS_TOKENS).map(([key, [token, light]]) => [key, style.getPropertyValue(token).trim() || light]),
  ) as CanvasPalette;
}
