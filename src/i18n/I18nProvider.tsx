import { createContext, useContext, type ReactNode } from "react";

import { setCurrentLang, type Lang } from "./lang";
import { pick, type Catalog, type Messages } from "./messages";

const LangContext = createContext<Lang>("vi");

/**
 * Renders its children in one language. Changing `lang` re-renders everything below, and
 * the format helpers switch with it.
 */
export function I18nProvider({ lang, children }: { lang: Lang; children: ReactNode }) {
  // Set during render, before any child formats a date or a duration in this pass.
  setCurrentLang(lang);
  return <LangContext.Provider value={lang}>{children}</LangContext.Provider>;
}

export function useLang(): Lang {
  return useContext(LangContext);
}

/** This area's messages in the language being rendered. */
export function useMessages<T extends Messages>(catalog: Catalog<T>): T {
  return pick(catalog, useLang());
}
