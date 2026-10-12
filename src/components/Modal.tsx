import { useEffect, useId, useRef, useState, type ReactNode } from "react";

interface Props {
  /** The heading, also the dialog's accessible name. */
  title: ReactNode;
  /** Extra classes on the dialog box, e.g. `wide`. */
  className?: string;
  /** Shown beside the title, e.g. a close button. */
  headerAction?: ReactNode;
  /** Escape and a press on the backdrop call this; without it they do nothing. */
  onClose?: () => void;
  children: ReactNode;
}

/**
 * A dialog over the window. It is `aria-modal`, so menu and shortcut actions other than
 * undo, redo and save stay off while it is open (`shouldRunAction`), and Tab cycles
 * through its own controls only. Opening moves focus into it (unless a child already took
 * it, e.g. with `autoFocus`), and closing returns focus to where it was.
 *
 * Escape and the backdrop close it like the Đóng button: a field being edited commits
 * first (its `onBlur` runs) instead of losing what was typed.
 */
export function Modal({ title, className, headerAction, onClose, children }: Props) {
  const titleId = useId();
  const boxRef = useRef<HTMLDivElement>(null);
  const onCloseRef = useRef(onClose);
  useEffect(() => {
    onCloseRef.current = onClose;
  });
  // Read during the first render: by the time effects run, a child may have taken focus.
  const [before] = useState(() => document.activeElement);

  useEffect(() => {
    const box = boxRef.current;
    if (!box) return;
    if (!box.contains(document.activeElement)) box.focus();
    // Listen on the document, not the box: after Enter commits a field, or a command
    // re-renders it, focus can drop to <body> and the box would no longer see keys.
    // A date picker stops Escape in the capture phase, so it closes first on its own.
    const onKey = (event: KeyboardEvent) => {
      if (event.defaultPrevented || !isTopmost(box)) return;
      if (event.key === "Escape" && !event.isComposing && onCloseRef.current) {
        event.preventDefault();
        closeCommitted(onCloseRef.current);
      } else if (event.key === "Tab") {
        keepFocusInside(box, event);
      }
    };
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("keydown", onKey);
      // StrictMode runs this cleanup once while the box is still mounted; only a real
      // unmount (box gone from the page) hands focus back.
      if (box.isConnected) return;
      if (before instanceof HTMLElement && before.isConnected) before.focus();
    };
  }, [before]);

  const heading = <h2 id={titleId}>{title}</h2>;
  return (
    <div
      className="modal"
      onMouseDown={(event) => {
        if (onClose && event.target === event.currentTarget) {
          event.preventDefault();
          closeCommitted(onClose);
        }
      }}
    >
      <div
        ref={boxRef}
        className={className ? `dialog ${className}` : "dialog"}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        // A click on text or a chart lands focus on the box instead of <body>.
        tabIndex={-1}
      >
        {headerAction ? (
          <div className="dialog-head">
            {heading}
            {headerAction}
          </div>
        ) : (
          heading
        )}
        {children}
      </div>
    </div>
  );
}

/** Blur the focused field so its `onBlur` commit runs, then close. */
function closeCommitted(onClose: () => void) {
  const active = document.activeElement;
  if (active instanceof HTMLElement) active.blur();
  onClose();
}

function isTopmost(box: HTMLElement): boolean {
  const open = document.querySelectorAll('[aria-modal="true"]');
  return open[open.length - 1] === box;
}

const CONTROLS =
  "button:not(:disabled), [href], input:not(:disabled), select:not(:disabled), textarea:not(:disabled), [tabindex]:not([tabindex='-1'])";

/** Tab and Shift+Tab wrap around the dialog's controls instead of leaving for the window. */
function keepFocusInside(box: HTMLElement, event: KeyboardEvent) {
  const active = document.activeElement;
  // A date picker opens in a portal and moves its own focus.
  if (active instanceof Element && active.closest(".picker-popover")) return;
  const controls = Array.from(box.querySelectorAll<HTMLElement>(CONTROLS));
  const first = controls[0];
  const last = controls[controls.length - 1];
  if (!first || !last) {
    event.preventDefault();
    box.focus();
    return;
  }
  const outside = !box.contains(active);
  const wrap = event.shiftKey ? outside || active === first || active === box : outside || active === last;
  if (wrap) {
    event.preventDefault();
    (event.shiftKey ? last : first).focus();
  }
}
