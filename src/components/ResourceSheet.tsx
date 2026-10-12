import { useMessages } from "../i18n";
import type { Calendar, Overallocation, Resource, ResourceKind } from "../types";
import { resourceSheetMessages } from "./ResourceSheet.i18n";

interface Props {
  resources: Resource[];
  calendars: Calendar[];
  overallocations: Overallocation[];
  onAdd: (kind: ResourceKind) => void;
  onChange: (resource: Resource) => void;
  onDelete: (uid: number) => void;
  onEditCalendar: (calendarId: number) => void;
  onCreateCalendar: (resource: Resource) => void;
}

export function ResourceSheet({
  resources,
  calendars,
  overallocations,
  onAdd,
  onChange,
  onDelete,
  onEditCalendar,
  onCreateCalendar,
}: Props) {
  const m = useMessages(resourceSheetMessages);
  const overloaded = new Set(overallocations.map((item) => item.resource_uid));
  return (
    <div className="resource-sheet">
      <div className="resource-tools">
        <button type="button" onClick={() => onAdd("person")}>{m.addPerson}</button>
        <button type="button" onClick={() => onAdd("machine")}>{m.addMachine}</button>
      </div>
      <table>
        <thead>
          <tr>
            <th>{m.name}</th>
            <th>{m.kind}</th>
            <th>{m.maxUnits}</th>
            <th>{m.calendar}</th>
            <th>{m.standardRate}</th>
            <th>{m.overtimeRate}</th>
            <th>{m.costPerUse}</th>
            <th>{m.overallocated}</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          {resources.map((resource) => (
            <tr key={resource.uid} className={overloaded.has(resource.uid) ? "over" : ""}>
              <td>
                <input
                  key={`${resource.uid}-name`}
                  defaultValue={resource.name}
                  onBlur={(event) => {
                    if (event.target.value !== resource.name) onChange({ ...resource, name: event.target.value });
                  }}
                />
              </td>
              <td>
                <select
                  value={resource.kind}
                  onChange={(event) => onChange({ ...resource, kind: event.target.value as ResourceKind })}
                >
                  <option value="person">{m.person}</option>
                  <option value="machine">{m.machine}</option>
                </select>
              </td>
              <td>
                <input
                  key={`${resource.uid}-units-${resource.max_units}`}
                  defaultValue={String(resource.max_units)}
                  onBlur={(event) => onChange({ ...resource, max_units: Number(event.target.value) || 0 })}
                />
              </td>
              <td>
                <div className="calendar-cell">
                  <select
                    aria-label={m.calendarOf(resource.name)}
                    value={resource.calendar_id ?? ""}
                    onChange={(event) =>
                      onChange({
                        ...resource,
                        calendar_id: event.target.value ? Number(event.target.value) : null,
                      })
                    }
                  >
                    <option value="">{m.projectCalendar}</option>
                    {calendars.map((calendar) => (
                      <option key={calendar.id} value={calendar.id}>{calendar.name}</option>
                    ))}
                  </select>
                  <button
                    type="button"
                    disabled={resource.calendar_id == null}
                    onClick={() => resource.calendar_id != null && onEditCalendar(resource.calendar_id)}
                  >
                    {m.editCalendar}
                  </button>
                  <button type="button" onClick={() => onCreateCalendar(resource)}>{m.newCalendar}</button>
                </div>
              </td>
              <td>
                <input
                  key={`${resource.uid}-rate-${resource.standard_rate}`}
                  defaultValue={String(resource.standard_rate)}
                  onBlur={(event) => onChange({ ...resource, standard_rate: Number(event.target.value) || 0 })}
                />
              </td>
              <td>
                <input
                  key={`${resource.uid}-ot-${resource.overtime_rate}`}
                  defaultValue={String(resource.overtime_rate)}
                  onBlur={(event) => onChange({ ...resource, overtime_rate: Number(event.target.value) || 0 })}
                />
              </td>
              <td>
                <input
                  key={`${resource.uid}-use-${resource.cost_per_use}`}
                  defaultValue={String(resource.cost_per_use)}
                  onBlur={(event) => onChange({ ...resource, cost_per_use: Number(event.target.value) || 0 })}
                />
              </td>
              <td className={overloaded.has(resource.uid) ? "bad" : ""}>
                {overloaded.has(resource.uid) ? m.yes : m.no}
              </td>
              <td>
                <button type="button" onClick={() => onDelete(resource.uid)}>{m.remove}</button>
              </td>
            </tr>
          ))}
          {resources.length === 0 && (
            <tr>
              <td colSpan={9} className="empty">{m.empty}</td>
            </tr>
          )}
        </tbody>
      </table>
    </div>
  );
}
