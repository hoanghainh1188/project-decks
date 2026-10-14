import { Check, Plus, X } from "lucide-react";
import { useRef } from "react";

import { formatUnits, hoursPerDay, parseUnits, showsUnits } from "../assignmentUnits";
import { useMessages } from "../i18n";
import type { AvailabilityPeriod, Command, Resource, Snapshot } from "../types";
import { commitFocusedField } from "../shortcuts";
import { usePendingList } from "../usePendingList";
import { Button } from "./Button";
import { DateKeyField } from "./DateTimeField";
import { Modal } from "./Modal";
import { resourceAvailabilityMessages } from "./ResourceAvailabilityDialog.i18n";

/** What a new period gets: a week at half time. */
const NEW_DAYS = 7;
const NEW_UNITS = 0.5;

const DATE_KEY = /^\d{4}-\d{2}-\d{2}$/;

/** A "YYYY-MM-DD" key `days` later, or `null` past the years a key can hold. */
function addDays(key: string, days: number): string | null {
  const [year, month, day] = key.split("-").map(Number);
  const date = new Date(0);
  // setUTCFullYear, unlike Date.UTC, keeps years below 100 as they are.
  date.setUTCFullYear(year, month - 1, day + days);
  const next = date.toISOString().slice(0, 10);
  return DATE_KEY.test(next) ? next : null;
}

/**
 * The days a resource has other units than its max units, like the Resource
 * Availability table of Project. A transaction like Lịch làm việc: edits apply as they
 * are made, Lưu keeps them and Hủy (✕, Escape) undoes back to where the dialog opened.
 */
export function ResourceAvailabilityDialog({
  resource,
  changed,
  error,
  today,
  onSave,
  onCancel,
  send,
}: {
  resource: Resource;
  changed: boolean;
  /** The last edit the engine refused. */
  error: string | null;
  /** Today as "YYYY-MM-DD": where a first new period starts. */
  today: string;
  onSave: () => void;
  onCancel: () => void;
  send: (command: Command) => Promise<Snapshot | void>;
}) {
  const a = useMessages(resourceAvailabilityMessages);
  const { latest, apply, settled } = usePendingList(resource.availability, (periods) =>
    send({ type: "set_resource_availability", uid: resource.uid, periods }),
  );
  // Rows show the periods the engine holds; a change made before its answer comes must
  // still find its period in the list sent last, where an earlier change may have moved
  // it (a removal above it) or replaced it (an edit of the same period).
  const replaced = useRef(new WeakMap<AvailabilityPeriod, AvailabilityPeriod>());
  const indexOf = (period: AvailabilityPeriod) => {
    let current = period;
    for (let next = replaced.current.get(current); next; next = replaced.current.get(current)) current = next;
    return latest().indexOf(current);
  };
  const periods = resource.availability;
  const last = periods[periods.length - 1];
  const openEnded = last != null && last.to == null;
  const nextFrom = (() => {
    const after = last?.to ? addDays(last.to, 1) : today;
    return after == null ? null : after > today ? after : today;
  })();

  function change(period: AvailabilityPeriod, patch: Partial<AvailabilityPeriod>) {
    const at = indexOf(period);
    if (at < 0) return;
    const current = latest();
    const next = { ...current[at], ...patch };
    replaced.current.set(current[at], next);
    void apply(current.map((item, index) => (index === at ? next : item)));
  }

  function remove(period: AvailabilityPeriod) {
    const at = indexOf(period);
    if (at >= 0) void apply(latest().filter((_, index) => index !== at));
  }

  function add() {
    const current = latest();
    const end = current[current.length - 1]?.to;
    const after = end ? addDays(end, 1) : today;
    const from = after == null ? null : after > today ? after : today;
    const to = from == null ? null : addDays(from, NEW_DAYS - 1);
    if (from != null && to != null) void apply([...current, { from, to, units: NEW_UNITS }]);
  }

  // Lưu leaves the field being typed in first, and closes only once the engine took it.
  async function save() {
    commitFocusedField();
    if (await settled()) onSave();
  }

  return (
    <Modal
      title={a.title(resource.name)}
      className="wide"
      onClose={onCancel}
      footer={
        <>
          {error ? (
            <span className="dialog-foot-note bad" role="alert">
              {error}
            </span>
          ) : (
            <span className="meta dialog-foot-note">{changed ? a.previewing : ""}</span>
          )}
          <Button Icon={X} onClick={onCancel}>{a.cancel}</Button>
          <Button variant="primary" Icon={Check} onClick={() => void save()}>{a.save}</Button>
        </>
      }
    >
      <p className="meta">{a.intro(resource.name, formatUnits(resource.max_units))}</p>
      {periods.length === 0 ? (
        <p>{a.none}</p>
      ) : (
        <table className="availability" aria-label={a.periods}>
          <thead>
            <tr>
              <th scope="col">{a.from}</th>
              <th scope="col">{a.to}</th>
              <th scope="col">{a.units}</th>
              <th scope="col">
                <span className="visually-hidden">{a.removeColumn}</span>
              </th>
            </tr>
          </thead>
          <tbody>
            {periods.map((period, index) => (
              // By place: an edit keeps every period where it is, so focus stays.
              <tr key={index}>
                <td>
                  <DateKeyField
                    optional
                    ariaLabel={a.fromOf(index + 1)}
                    placeholder={a.noLimit}
                    value={period.from ?? ""}
                    onCommit={(key) => change(period, { from: key || null })}
                  />
                </td>
                <td>
                  <DateKeyField
                    optional
                    ariaLabel={a.toOf(index + 1)}
                    placeholder={a.noLimit}
                    value={period.to ?? ""}
                    onCommit={(key) => change(period, { to: key || null })}
                  />
                </td>
                <td>
                  <span className="availability-units">
                    <input
                      key={period.units}
                      aria-label={a.unitsOf(index + 1)}
                      aria-describedby={`availability-hours-${index}`}
                      defaultValue={formatUnits(period.units)}
                      onBlur={(event) => {
                        const text = event.target.value;
                        const units = parseUnits(text);
                        if (units == null) {
                          event.target.value = formatUnits(period.units);
                        } else if (!showsUnits(text, period.units)) {
                          change(period, { units });
                        }
                      }}
                    />
                    <span className="meta" id={`availability-hours-${index}`}>{hoursPerDay(period.units)}</span>
                  </span>
                </td>
                <td>
                  <Button
                    Icon={X}
                    className="btn-icon"
                    aria-label={a.remove(index + 1)}
                    onClick={() => remove(period)}
                  />
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      <div className="row">
        <Button
          Icon={Plus}
          disabled={openEnded || nextFrom == null}
          title={openEnded ? a.addAfterOpen : undefined}
          onClick={add}
        >
          {a.add}
        </Button>
      </div>
    </Modal>
  );
}
