import { Plus, Trash2 } from "lucide-react";

import { useMessages } from "../../i18n";
import type { StatusClass } from "../../types";
import { Button } from "../Button";
import { STATUS_CLASSES, type SourceForm } from "./trackerSource";
import { trackerDialogMessages } from "./TrackerDialog.i18n";

const CLASS_LABEL = { not_started: "notStarted", in_progress: "inProgress", done: "done" } as const;

/** The form of the source: where the tracker is, which issues, the key and the status map. */
export function TrackerSetup({
  form,
  onForm,
  keyInput,
  onKeyInput,
  hasKey,
  onForgetKey,
  disabled,
}: {
  form: SourceForm;
  onForm: (form: SourceForm) => void;
  keyInput: string;
  onKeyInput: (key: string) => void;
  hasKey: boolean;
  onForgetKey: () => void;
  disabled: boolean;
}) {
  const m = useMessages(trackerDialogMessages);
  const setStatus = (index: number, change: Partial<SourceForm["statuses"][number]>) =>
    onForm({ ...form, statuses: form.statuses.map((item, at) => (at === index ? { ...item, ...change } : item)) });
  return (
    <div className="tracker-setup">
      <label>
        {m.site}
        <input
          value={form.site}
          disabled={disabled}
          placeholder="team.backlog.com"
          autoCapitalize="off"
          spellCheck={false}
          onChange={(event) => onForm({ ...form, site: event.target.value })}
        />
        <span className="meta">{m.siteHint}</span>
      </label>
      <label>
        {m.projectKey}
        <input
          value={form.projectKey}
          disabled={disabled}
          placeholder="PROJ"
          spellCheck={false}
          onChange={(event) => onForm({ ...form, projectKey: event.target.value })}
        />
      </label>
      <label>
        {m.filter}
        <input
          value={form.filter}
          disabled={disabled}
          spellCheck={false}
          onChange={(event) => onForm({ ...form, filter: event.target.value })}
        />
        <span className="meta">{m.filterHint}</span>
      </label>
      <div className="tracker-key">
        <label>
          {m.apiKey}
          <input
            type="password"
            value={keyInput}
            disabled={disabled}
            placeholder={m.keyPlaceholder}
            autoComplete="off"
            spellCheck={false}
            onChange={(event) => onKeyInput(event.target.value)}
          />
        </label>
        <span className="meta" role="status">
          {hasKey ? m.keySaved : m.keyNone}
        </span>
        {hasKey && (
          <Button Icon={Trash2} disabled={disabled} onClick={onForgetKey}>
            {m.forgetKey}
          </Button>
        )}
      </div>
      <fieldset className="tracker-statuses">
        <legend>{m.statuses}</legend>
        <p className="meta">{m.statusHint}</p>
        {form.statuses.map((item, index) => (
          <div className="tracker-status" key={index}>
            <input
              aria-label={m.statusName}
              value={item.status}
              disabled={disabled}
              onChange={(event) => setStatus(index, { status: event.target.value })}
            />
            <select
              aria-label={`${m.statusName}: ${item.status}`}
              value={item.class}
              disabled={disabled}
              onChange={(event) => setStatus(index, { class: event.target.value as StatusClass })}
            >
              {STATUS_CLASSES.map((kind) => (
                <option key={kind} value={kind}>
                  {m[CLASS_LABEL[kind]]}
                </option>
              ))}
            </select>
            <Button
              Icon={Trash2}
              aria-label={m.removeStatus(item.status)}
              disabled={disabled}
              onClick={() => onForm({ ...form, statuses: form.statuses.filter((_, at) => at !== index) })}
            />
          </div>
        ))}
        <Button
          Icon={Plus}
          disabled={disabled}
          onClick={() => onForm({ ...form, statuses: [...form.statuses, { status: "", class: "in_progress" }] })}
        >
          {m.addStatus}
        </Button>
      </fieldset>
    </div>
  );
}
