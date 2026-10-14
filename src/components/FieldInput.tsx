import { useEffect, useState } from "react";

import { formatFieldValue, MAX_TEXT, parseFieldValue } from "../fieldValues";
import { useMessages } from "../i18n";
import type { FieldDef, FieldValue } from "../types";
import { fieldInputMessages } from "./FieldInput.i18n";

interface Props {
  id?: string;
  field: FieldDef;
  value: FieldValue | undefined;
  ariaLabel: string;
  className?: string;
  /** A new value, or null to clear it. Not called when the text does not fit the field. */
  onCommit: (value: FieldValue | null) => void;
}

/**
 * One custom field value, edited in place: a list for a choice field, text for the
 * others. Text commits on blur, only when it changes the value; text a number field
 * cannot read is put back and marked. A new value from elsewhere (undo, fill-down,
 * another view) shows at once while the field is not being typed in.
 */
export function FieldInput({ id, field, value, ariaLabel, className, onCommit }: Props) {
  const m = useMessages(fieldInputMessages);
  const shown = formatFieldValue(value);
  const [text, setText] = useState(shown);
  const [editing, setEditing] = useState(false);
  const [rejected, setRejected] = useState(false);
  // A value set elsewhere replaces a rejected one, so the mark goes with it.
  useEffect(() => setRejected(false), [value]);

  if (field.kind.type === "choice") {
    const listed = value === undefined || field.kind.options.includes(String(value));
    return (
      <select
        id={id}
        className={className}
        aria-label={ariaLabel}
        value={value === undefined ? "" : String(value)}
        onChange={(event) => onCommit(event.target.value === "" ? null : event.target.value)}
      >
        <option value="">{m.empty}</option>
        {field.kind.options.map((option) => (
          <option key={option} value={option}>
            {option}
          </option>
        ))}
        {/* A value from before the list changed stays visible until it is replaced. */}
        {!listed && <option value={String(value)}>{String(value)}</option>}
      </select>
    );
  }

  const classes = [className, rejected ? "bad" : ""].filter(Boolean).join(" ");
  return (
    <input
      id={id}
      className={classes || undefined}
      aria-label={ariaLabel}
      aria-invalid={rejected || undefined}
      title={rejected ? m.notANumber : undefined}
      inputMode={field.kind.type === "number" ? "decimal" : undefined}
      maxLength={field.kind.type === "text" ? MAX_TEXT : undefined}
      value={editing ? text : shown}
      onChange={(event) => setText(event.target.value)}
      onFocus={() => {
        setEditing(true);
        setText(shown);
        setRejected(false);
      }}
      onBlur={() => {
        setEditing(false);
        if (text === shown) return;
        const next = parseFieldValue(field, text);
        setRejected(next === undefined);
        // "5,0" for 5, or the same text again, is no edit: no undo step, no dirty file.
        if (next !== undefined && next !== (value ?? null)) onCommit(next);
      }}
    />
  );
}
