import { useState } from "react";

import { formatInstant, parseInstant } from "../format";

interface DateTimeFieldProps {
  value: number | null;
  optional?: boolean;
  disabled?: boolean;
  ariaLabel?: string;
  onCommit: (ms: number | null) => void;
}

export function DateTimeField({ value, optional, disabled, ariaLabel, onCommit }: DateTimeFieldProps) {
  const shown = value == null ? "" : formatInstant(value);
  const [text, setText] = useState(shown);
  const [editing, setEditing] = useState(false);

  function commit() {
    setEditing(false);
    const raw = text.trim();
    if (!raw) {
      if (optional && value != null) onCommit(null);
      return;
    }
    const parsed = parseInstant(raw);
    if (parsed !== null && parsed !== value) onCommit(parsed);
  }

  return (
    <span className="datetime-field">
      <input
        type="text"
        inputMode="numeric"
        autoComplete="off"
        spellCheck={false}
        disabled={disabled}
        aria-label={ariaLabel}
        value={editing ? text : shown}
        onChange={(event) => setText(event.target.value)}
        onFocus={() => {
          setEditing(true);
          setText(shown);
        }}
        onBlur={commit}
        onKeyDown={(event) => {
          if (event.key === "Enter") event.currentTarget.blur();
        }}
      />
      {optional && value != null && (
        <button type="button" disabled={disabled} onClick={() => onCommit(null)}>
          Xóa
        </button>
      )}
    </span>
  );
}
