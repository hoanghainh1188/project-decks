import { useEffect, useRef } from "react";

import { isEditableTarget } from "./shortcuts";

/** Whether Copy and Paste should act on tasks: no text field or dialog has them, and no page text is selected. */
function forTasks(): boolean {
  if (isEditableTarget(document.activeElement)) return false;
  if (document.querySelector('[aria-modal="true"]')) return false;
  const selection = window.getSelection();
  return !selection || selection.isCollapsed;
}

/**
 * Edit → Copy / Paste and Cmd+C / Cmd+V outside a text field copy and paste the selected
 * tasks. The native items stay native so fields keep their own copy and paste; the
 * webview turns them into the page's `copy` and `paste` events, which land here.
 * Cancelling `beforecopy` / `beforepaste` lets WebKit enable the items with no text
 * selected. `copyText` gives the plain text other apps receive (the task names).
 * A paste whose text is not what this app copied last goes to `pasteText` first, which
 * answers whether it took it (rows copied from a spreadsheet); otherwise the tasks paste.
 */
export function useClipboardEvents(
  dispatch: (id: string) => void,
  copyText: () => string,
  pasteText?: (text: string) => boolean,
): void {
  const text = useRef(copyText);
  text.current = copyText;
  const paste = useRef(pasteText);
  paste.current = pasteText;
  const copied = useRef<string | null>(null);
  useEffect(() => {
    const enable = (event: Event) => {
      if (forTasks()) event.preventDefault();
    };
    const onCopy = (event: ClipboardEvent) => {
      if (!forTasks()) return;
      event.preventDefault();
      copied.current = text.current();
      event.clipboardData?.setData("text/plain", copied.current);
      dispatch("copy-task");
    };
    const onPaste = (event: ClipboardEvent) => {
      if (!forTasks()) return;
      event.preventDefault();
      const pasted = event.clipboardData?.getData("text/plain") ?? "";
      if (pasted !== "" && pasted !== copied.current && paste.current?.(pasted)) return;
      dispatch("paste-task");
    };
    document.addEventListener("beforecopy", enable);
    document.addEventListener("beforepaste", enable);
    document.addEventListener("copy", onCopy);
    document.addEventListener("paste", onPaste);
    return () => {
      document.removeEventListener("beforecopy", enable);
      document.removeEventListener("beforepaste", enable);
      document.removeEventListener("copy", onCopy);
      document.removeEventListener("paste", onPaste);
    };
  }, [dispatch]);
}
