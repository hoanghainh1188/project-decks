import { useEffect, useId, useRef, useState, type KeyboardEvent } from "react";

export interface MenuItem {
  label: string;
  onSelect: () => void;
  disabled?: boolean;
  /** Why the item is off, or what it does. */
  title?: string;
}

interface Props {
  label: string;
  items: MenuItem[];
  disabled?: boolean;
  title?: string;
}

/**
 * A toolbar button that opens a short menu, following the WAI-ARIA menu button pattern:
 * opening focuses the first enabled item, arrows move, Escape closes back to the button,
 * and Tab or a click elsewhere closes it.
 */
export function MenuButton({ label, items, disabled, title }: Props) {
  const [open, setOpen] = useState(false);
  const buttonRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const menuId = useId();

  // A command that turns off while its menu is open closes the menu.
  useEffect(() => {
    if (disabled) setOpen(false);
  }, [disabled]);

  useEffect(() => {
    if (!open) return;
    const first = menuRef.current?.querySelector<HTMLButtonElement>("[role=menuitem]:not(:disabled)");
    first?.focus();
    const close = (event: MouseEvent) => {
      const root = buttonRef.current?.parentElement;
      if (root && !root.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", close);
    return () => document.removeEventListener("mousedown", close);
  }, [open]);

  const enabledItems = () =>
    Array.from(menuRef.current?.querySelectorAll<HTMLButtonElement>("[role=menuitem]:not(:disabled)") ?? []);

  const onMenuKey = (event: KeyboardEvent<HTMLDivElement>) => {
    const list = enabledItems();
    const at = list.indexOf(document.activeElement as HTMLButtonElement);
    const focus = (index: number) => list[(index + list.length) % list.length]?.focus();
    if (event.key === "ArrowDown") focus(at + 1);
    else if (event.key === "ArrowUp") focus(at - 1);
    else if (event.key === "Home") focus(0);
    else if (event.key === "End") focus(list.length - 1);
    else if (event.key === "Escape") {
      setOpen(false);
      buttonRef.current?.focus();
    } else if (event.key === "Tab") {
      // Move focus out of the menu first, so Tab continues from the button.
      buttonRef.current?.focus();
      setOpen(false);
    } else return;
    if (event.key !== "Tab") event.preventDefault();
    // Keep the app's shortcuts from also acting on these keys.
    event.stopPropagation();
  };

  return (
    <div className="menu-button">
      <button
        ref={buttonRef}
        type="button"
        disabled={disabled}
        title={title}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={open ? menuId : undefined}
        className={open ? "on" : ""}
        onClick={() => setOpen((current) => !current)}
        onKeyDown={(event) => {
          if (event.key === "ArrowDown" && !open) {
            event.preventDefault();
            setOpen(true);
          }
        }}
      >
        {label} <span aria-hidden="true">▾</span>
      </button>
      {open && (
        <div id={menuId} ref={menuRef} className="menu" role="menu" aria-label={label} onKeyDown={onMenuKey}>
          {items.map((item, index) => (
            <button
              key={`${index}-${item.label}`}
              type="button"
              role="menuitem"
              disabled={item.disabled}
              title={item.title}
              onClick={() => {
                setOpen(false);
                buttonRef.current?.focus();
                item.onSelect();
              }}
            >
              {item.label}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
