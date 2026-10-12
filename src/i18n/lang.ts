/** The interface languages. Vietnamese is the original; English follows it key for key. */
export type Lang = "vi" | "en";

export const LANGS: readonly Lang[] = ["vi", "en"];

export function isLang(value: unknown): value is Lang {
  return value === "vi" || value === "en";
}

// The language the format helpers (dates, durations, money) read. I18nProvider keeps it in
// step with the language React renders, so plain functions need no extra argument.
// Vietnamese by default, which keeps tests that do not set a language unchanged.
let current: Lang = "vi";

export function getLang(): Lang {
  return current;
}

export function setCurrentLang(lang: Lang): void {
  current = lang;
}

/** First launch: Vietnamese when the system prefers it, English otherwise. */
export function detectLang(preferred: readonly string[]): Lang {
  const first = (preferred[0] ?? "").toLowerCase();
  return first === "vi" || first.startsWith("vi-") ? "vi" : "en";
}
