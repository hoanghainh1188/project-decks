import { useCallback, useEffect, useState } from "react";

import App from "./App";
import { getLanguage, getTheme, inTauri, setLanguage } from "./api";
import { detectLang, I18nProvider, type Lang } from "./i18n";
import { applyTheme } from "./theme";

function systemLang(): Lang {
  return detectLang(navigator.languages?.length ? navigator.languages : [navigator.language]);
}

/**
 * Picks the interface language and the theme before the app renders: the user's saved
 * choices, or the system's. The backend follows along, so the native menu and dialogs match.
 */
export function Root() {
  // Outside Tauri there is no saved choice to wait for.
  const [lang, setLang] = useState<Lang | null>(() => (inTauri() ? null : systemLang()));

  useEffect(() => {
    if (!inTauri()) return;
    let cancelled = false;
    void (async () => {
      const [saved, theme] = await Promise.all([
        getLanguage().catch(() => null),
        getTheme().catch(() => "system" as const),
      ]);
      if (!cancelled) applyTheme(theme);
      const next = saved ?? systemLang();
      // Wait for the backend to switch first, so a dialog it opens at launch (restoring an
      // autosave) is already in this language. Not remembered when it follows the system.
      await setLanguage(next, saved != null).catch(() => undefined);
      if (!cancelled) setLang(next);
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  // The interface switches at once; a failure to tell the backend or store the choice is
  // returned to the caller, which shows it.
  const change = useCallback(async (next: Lang) => {
    setLang(next);
    if (inTauri()) await setLanguage(next, true);
  }, []);

  if (!lang) return null;
  return (
    <I18nProvider lang={lang} onLangChange={change}>
      <App />
    </I18nProvider>
  );
}
