import { useMessages } from "../i18n";
import type { Command, ForecastSettings, Project, Snapshot, Strength } from "../types";
import { forecastTabMessages } from "./ForecastTab.i18n";

type Send = (command: Command) => Promise<Snapshot | void>;

const STRENGTHS: Strength[] = ["none", "low", "medium", "high"];

/**
 * Dự báo, inside Thông tin dự án: how tasks' effort varies together in the finish
 * forecast. Every change sends the whole settings; Hủy undoes them with the rest of the
 * dialog.
 */
export function ForecastTab({ project, send }: { project: Project; send: Send }) {
  const m = useMessages(forecastTabMessages);
  const settings = project.forecast;
  const fields = project.fields.filter((field) => field.kind.type !== "number");
  const change = (next: Partial<ForecastSettings>) =>
    void send({ type: "set_forecast_settings", settings: { ...settings, ...next } });
  const strength = (id: string, label: string, value: Strength, onChange: (value: Strength) => void, disabled = false) => (
    <>
      <label htmlFor={id}>{label}</label>
      <select id={id} value={value} disabled={disabled} onChange={(event) => onChange(event.target.value as Strength)}>
        {STRENGTHS.map((item) => (
          <option key={item} value={item}>
            {m[item]}
          </option>
        ))}
      </select>
    </>
  );
  return (
    <div className="forecast-tab">
      <p className="meta">{m.intro}</p>
      <section>
      <label htmlFor="forecast-field">{m.field}</label>
      <select
        id="forecast-field"
        value={settings.field ?? ""}
        aria-describedby="forecast-field-note"
        onChange={(event) => change({ field: event.target.value ? Number(event.target.value) : null })}
      >
        <option value="">{m.noField}</option>
        {fields.map((field) => (
          <option key={field.id} value={field.id}>
            {field.name}
          </option>
        ))}
      </select>
      <p className="meta" id="forecast-field-note">
        {m.fieldNote}
      </p>
      </section>
      <section>
        {strength("forecast-group", m.group, settings.group, (group) => change({ group }), settings.field == null)}
        {settings.field == null && <p className="meta">{m.groupNeedsField}</p>}
      </section>
      <section>
        {strength("forecast-common", m.common, settings.common, (common) => change({ common }))}
        <p className="meta">{m.commonNote}</p>
      </section>
    </div>
  );
}
