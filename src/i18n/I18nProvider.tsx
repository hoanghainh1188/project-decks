import { createContext, useContext, type ReactNode } from "react";

import { setCurrentLang, type Lang } from "./lang";
import { pick, type Catalog, type Messages } from "./messages";

const LangContext = createContext<Lang>("vi");
const SetLangContext = createContext<(lang: Lang) => void>(() => {});

interface Props {
  lang: Lang;
  /** Called by `useSetLang()`, e.g. from the language menu; the owner stores the choice. */
  onLangChange?: (lang: Lang) => void;
  children: ReactNode;
}

/**
 * Renders its children in one language. Changing `lang` re-renders everything below, and
 * the format helpers switch with it.
 */
export function I18nProvider({ lang, onLangChange, children }: Props) {
  // Set during render, before any child formats a date or a duration in this pass.
  setCurrentLang(lang);
  return (
    <SetLangContext.Provider value={onLangChange ?? noop}>
      <LangContext.Provider value={lang}>{children}</LangContext.Provider>
    </SetLangContext.Provider>
  );
}

function noop() {}

export function useLang(): Lang {
  return useContext(LangContext);
}

/** Switch the interface language. */
export function useSetLang(): (lang: Lang) => void {
  return useContext(SetLangContext);
}

/** This area's messages in the language being rendered. */
export function useMessages<T extends Messages>(catalog: Catalog<T>): T {
  return pick(catalog, useLang());
}
