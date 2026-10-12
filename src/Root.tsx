import { useCallback, useEffect, useState } from "react";

import App from "./App";
import { getLanguage, inTauri, setLanguage } from "./api";
import { detectLang, I18nProvider, type Lang } from "./i18n";

function systemLang(): Lang {
  return detectLang(navigator.languages?.length ? navigator.languages : [navigator.language]);
}

/**
 * Picks the interface language before the app renders: the user's saved choice, or the
 * system language. The backend follows along, so the native menu and dialogs match.
 */
export function Root() {
  // Outside Tauri there is no saved choice to wait for.
  const [lang, setLang] = useState<Lang | null>(() => (inTauri() ? null : systemLang()));

  useEffect(() => {
    if (!inTauri()) return;
    let cancelled = false;
    getLanguage()
      .catch(() => null)
      .then((saved) => {
        if (cancelled) return;
        const next = saved ?? systemLang();
        setLang(next);
        // Not remembered when it only follows the system.
        void setLanguage(next, saved != null).catch(() => undefined);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const change = useCallback((next: Lang) => {
    setLang(next);
    if (inTauri()) void setLanguage(next, true).catch(() => undefined);
  }, []);

  if (!lang) return null;
  return (
    <I18nProvider lang={lang} onLangChange={change}>
      <App />
    </I18nProvider>
  );
}
