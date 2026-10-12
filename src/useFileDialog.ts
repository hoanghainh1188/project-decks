import { useCallback, useRef, useState } from "react";

import { inTauri, openPending } from "./api";
import { appMessages } from "./App.i18n";
import { pick } from "./i18n";
import type { Snapshot } from "./types";

type Work = () => Promise<Snapshot | void | null>;

export interface FileDialogs {
  /** Run `work`, which opens a native dialog, unless one is already open. */
  startFileDialog: (work: Work) => void;
  /** Open a file the system sent (`open_pending`) now, or once the open dialog closes. */
  openFileWhenFree: () => void;
  /** True while a native dialog is open; toolbar commands that need one wait. */
  fileDialogBusy: boolean;
}

/**
 * One native file chooser at a time. A second click is ignored so it cannot open another
 * dialog while the first still owns the macOS run loop. `run` applies the result.
 */
export function useFileDialog(
  run: (work: Work) => Promise<Snapshot | void>,
  setError: (message: string) => void,
): FileDialogs {
  const busyRef = useRef(false);
  const [fileDialogBusy, setFileDialogBusy] = useState(false);
  // A file the system asked to open while a dialog was up; opened when it closes.
  const openWhenFreeRef = useRef(false);
  const openFileWhenFreeRef = useRef<() => void>(() => {});

  const startFileDialog = useCallback(
    (work: Work) => {
      if (busyRef.current) return;
      if (!inTauri()) {
        setError(pick(appMessages).needsTauri);
        return;
      }
      busyRef.current = true;
      setFileDialogBusy(true);
      void (async () => {
        try {
          await run(work);
        } finally {
          busyRef.current = false;
          setFileDialogBusy(false);
          if (openWhenFreeRef.current) {
            openWhenFreeRef.current = false;
            openFileWhenFreeRef.current();
          }
        }
      })();
    },
    [run, setError],
  );

  const openFileWhenFree = useCallback(() => {
    if (busyRef.current) openWhenFreeRef.current = true;
    else startFileDialog(openPending);
  }, [startFileDialog]);
  openFileWhenFreeRef.current = openFileWhenFree;

  return { startFileDialog, openFileWhenFree, fileDialogBusy };
}
