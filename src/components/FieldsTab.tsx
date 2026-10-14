import { Plus, Trash2, X } from "lucide-react";
import { useRef, useState } from "react";

import { distinctValues, valueKey, valueOf } from "../fieldValues";
import { useMessages } from "../i18n";
import type { Command, FieldDef, FieldKind, Project, Snapshot } from "../types";
import { Button } from "./Button";
import { fieldsTabMessages } from "./FieldsTab.i18n";

type Send = (command: Command) => Promise<Snapshot | void>;
type KindType = FieldKind["type"];
const KINDS: KindType[] = ["text", "number", "choice"];

/** Microsoft Project's limits, which the engine keeps so every field survives MSPDI. */
const MAX_TEXT = 30;
const MAX_NUMBER = 20;

/**
 * Custom task fields, inside Thông tin dự án: each edit applies at once (the sheet shows
 * it), and Hủy undoes every one of them with the rest of the dialog.
 */
export function FieldsTab({ project, send }: { project: Project; send: Send }) {
  const f = useMessages(fieldsTabMessages);
  const room = slotsLeft(project.fields);
  const full = room.text === 0;

  function addField() {
    const taken = new Set(project.fields.map((field) => field.name.toLowerCase()));
    let n = project.fields.length + 1;
    while (taken.has(f.newField(n).toLowerCase())) n += 1;
    void send({ type: "add_field", name: f.newField(n), kind: { type: "text" } });
  }

  return (
    <>
      <p className="meta">{f.intro}</p>
      {project.fields.length === 0 && <p>{f.none}</p>}
      <ul className="field-list">
        {project.fields.map((field) => (
          <li key={field.id}>
            <FieldEditor field={field} project={project} send={send} />
          </li>
        ))}
      </ul>
      <div className="row">
        <Button Icon={Plus} disabled={full} title={full ? f.textLimit(MAX_TEXT) : undefined} onClick={addField}>
          {f.add}
        </Button>
        <span className="meta">{f.limits(MAX_TEXT, MAX_NUMBER)}</span>
      </div>
    </>
  );
}

/** Text-or-list and number slots still free (Text1–30, Number1–20). */
function slotsLeft(fields: FieldDef[]): { text: number; number: number } {
  const numbers = fields.filter((field) => field.kind.type === "number").length;
  return { text: MAX_TEXT - (fields.length - numbers), number: MAX_NUMBER - numbers };
}

function FieldEditor({ field, project, send }: { field: FieldDef; project: Project; send: Send }) {
  const f = useMessages(fieldsTabMessages);
  const used = project.tasks.filter((task) => valueOf(task, field) !== undefined).length;
  const room = slotsLeft(project.fields);
  const update = (kind: FieldKind, name = field.name, renamed: { from: string; to: string }[] = []) =>
    send({ type: "update_field", id: field.id, name, kind, renamed_options: renamed });
  // The field's own slot counts as free when it changes kind within its group.
  const kindOpen = (type: KindType) =>
    type === field.kind.type ||
    (type === "number" ? field.kind.type === "number" || room.number > 0 : field.kind.type !== "number" || room.text > 0);

  function changeKind(type: KindType) {
    if (type === field.kind.type) return;
    if (type !== "choice") return void update({ type });
    // A list starts from the values tasks already have, so none of them is lost.
    const options = distinctValues(field, project.tasks).map((value) => valueKey(value));
    void update({ type, options: options.length > 0 ? options : [f.firstOption] });
  }

  return (
    <div className="field-editor" role="group" aria-label={field.name}>
      <div className="row">
        <input
          aria-label={f.name}
          className="field-name"
          maxLength={50}
          // Shows the name again after undo or redo inside the dialog.
          key={field.name}
          defaultValue={field.name}
          onBlur={(event) => {
            const input = event.target;
            const name = input.value.trim();
            if (!name || name === field.name) {
              input.value = field.name;
              return;
            }
            // Refused (a name another field has): the stored name stays, so show it again.
            void update(field.kind, name).then((snapshot) => {
              if (!snapshot) input.value = field.name;
            });
          }}
        />
        <select aria-label={f.kind} value={field.kind.type} onChange={(event) => changeKind(event.target.value as KindType)}>
          {KINDS.map((type) => (
            <option key={type} value={type} disabled={!kindOpen(type)}>
              {f[type]}
            </option>
          ))}
        </select>
        <span className="meta">{f.used(used)}</span>
        <Button variant="danger" Icon={Trash2} title={f.deleteTip} onClick={() => void send({ type: "delete_field", id: field.id })}>
          {f.delete}
        </Button>
      </div>
      {field.kind.type === "choice" && (
        <Choices
          options={field.kind.options}
          onChange={(options, renamed) => update({ type: "choice", options }, field.name, renamed)}
        />
      )}
    </div>
  );
}

/**
 * The choices of a list field. Renaming one in place keeps the tasks that have it. Each
 * change starts from the list as the last change left it, not as last drawn: blurring a
 * renamed choice by clicking a ✕ (its own or another's) sends both, the second after the
 * first. A refused change goes back to the list the engine holds.
 */
function Choices({
  options,
  onChange,
}: {
  options: string[];
  onChange: (options: string[], renamed: { from: string; to: string }[]) => Promise<Snapshot | void>;
}) {
  const f = useMessages(fieldsTabMessages);
  const [draft, setDraft] = useState("");
  // The list the next change starts from: the last one sent. A new snapshot brings the
  // field's options, which replace it only once no change is still on its way (a
  // re-render for typing, or the answer to an earlier change, must not drop one).
  const latest = useRef(options);
  const seen = useRef(options);
  const pending = useRef(0);
  // A choice renamed by a change still on its way, by the name it is drawn with.
  const renamedTo = useRef(new Map<string, string>());
  if (seen.current !== options) {
    seen.current = options;
    if (pending.current === 0) {
      latest.current = options;
      renamedTo.current.clear();
    }
  }
  const change = async (next: string[], renamed: { from: string; to: string }[] = []) => {
    latest.current = next;
    pending.current += 1;
    try {
      const snapshot = await onChange(next, renamed);
      if (!snapshot) {
        latest.current = seen.current;
        renamedTo.current.clear();
      }
      return snapshot;
    } finally {
      pending.current -= 1;
    }
  };

  function add() {
    const option = draft.trim();
    if (!option || latest.current.includes(option)) return;
    // A refused choice stays in the box to be fixed.
    void change([...latest.current, option]).then((snapshot) => {
      if (snapshot) setDraft("");
    });
  }

  return (
    <div className="choices">
      <ul>
        {options.map((option, index) => (
          <li key={option}>
            <input
              aria-label={f.choiceN(index + 1)}
              defaultValue={option}
              maxLength={255}
              onBlur={(event) => {
                const input = event.target;
                const next = input.value.trim();
                if (!next || next === option || latest.current.includes(next)) {
                  input.value = option;
                  return;
                }
                const current = renamedTo.current.get(option) ?? option;
                const renamed = latest.current.map((item) => (item === current ? next : item));
                renamedTo.current.set(option, next);
                void change(renamed, [{ from: current, to: next }]).then((snapshot) => {
                  if (!snapshot) input.value = option;
                });
              }}
            />
            <Button
              Icon={X}
              className="btn-icon"
              aria-label={f.removeChoice(option)}
              title={f.removeChoiceTip}
              disabled={options.length === 1}
              onClick={() => {
                const current = renamedTo.current.get(option) ?? option;
                void change(latest.current.filter((item) => item !== current));
              }}
            />
          </li>
        ))}
      </ul>
      <div className="row">
        <input
          aria-label={f.newChoice}
          placeholder={f.newChoice}
          value={draft}
          maxLength={255}
          onChange={(event) => setDraft(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              event.preventDefault();
              add();
            }
          }}
        />
        <Button Icon={Plus} disabled={!draft.trim() || options.includes(draft.trim())} onClick={add}>
          {f.addChoice}
        </Button>
      </div>
    </div>
  );
}
