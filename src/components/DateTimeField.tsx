import { datetimeLocalToInstant, instantToDatetimeLocal } from "../format";

interface DateTimeFieldProps {
  value: number | null;
  optional?: boolean;
  disabled?: boolean;
  ariaLabel?: string;
  onCommit: (ms: number | null) => void;
}

export function DateTimeField({ value, optional, disabled, ariaLabel, onCommit }: DateTimeFieldProps) {
  const shown = value == null ? "" : instantToDatetimeLocal(value);

  function commit(raw: string, allowEmpty: boolean) {
    if (!raw) {
      if (allowEmpty && optional && value != null) onCommit(null);
      return;
    }
    const parsed = datetimeLocalToInstant(raw);
    if (parsed !== null && parsed !== value) onCommit(parsed);
  }

  return (
    <span className="datetime-field">
      <input
        type="datetime-local"
        step={60}
        disabled={disabled}
        aria-label={ariaLabel}
        value={shown}
        onChange={(event) => commit(event.target.value, false)}
        onBlur={(event) => commit(event.target.value, true)}
      />
      {optional && value != null && (
        <button type="button" disabled={disabled} onClick={() => onCommit(null)}>
          Xóa
        </button>
      )}
    </span>
  );
}
