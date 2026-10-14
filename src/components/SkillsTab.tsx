import { Plus, X } from "lucide-react";
import { useState, type FormEvent } from "react";

import { useMessages } from "../i18n";
import type { Command, Project, Resource, SkillDef, Snapshot } from "../types";
import { usePendingList } from "../usePendingList";
import { skillsTabMessages } from "./AssignmentDialog.i18n";
import { Button } from "./Button";

type Send = (command: Command) => Promise<Snapshot | void>;

/** The engine's limit (`skills::MAX_SKILL_NAME_CHARS`). */
const MAX_NAME = 50;

/**
 * The project's skills (add, rename, delete) and who has which, as a grid of checkboxes.
 * Each change is its own command; Hủy of the dialog undoes them all.
 */
export function SkillsTab({ project, send }: { project: Project; send: Send }) {
  const s = useMessages(skillsTabMessages);
  const [draft, setDraft] = useState("");
  const comma = draft.includes(",");

  async function add(event: FormEvent) {
    event.preventDefault();
    const name = draft.trim();
    if (!name || comma) return;
    if (await send({ type: "add_skill", name })) setDraft("");
  }

  return (
    <>
      <p className="meta">{s.intro}</p>
      {project.skills.length === 0 ? (
        <p>{s.none}</p>
      ) : (
        <ul className="skill-list" role="list" aria-label={s.list}>
          {project.skills.map((skill) => (
            <SkillItem key={skill.id} skill={skill} send={send} />
          ))}
        </ul>
      )}
      <form className="row" onSubmit={(event) => void add(event)}>
        <input
          aria-label={s.newName}
          placeholder={s.newPlaceholder}
          maxLength={MAX_NAME}
          value={draft}
          aria-invalid={comma}
          className={comma ? "bad" : undefined}
          title={comma ? s.noComma : undefined}
          onChange={(event) => setDraft(event.target.value)}
        />
        <Button type="submit" Icon={Plus} disabled={!draft.trim() || comma}>
          {s.add}
        </Button>
      </form>
      {project.skills.length > 0 &&
        (project.resources.length === 0 ? (
          <p>{s.noPeople}</p>
        ) : (
          <div className="skill-grid-wrap">
            <table className="skill-grid" aria-label={s.people}>
              <thead>
                <tr>
                  <th scope="col">{s.person}</th>
                  {project.skills.map((skill) => (
                    <th key={skill.id} scope="col">
                      {skill.name}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {project.resources.map((resource) => (
                  <SkillRow key={resource.uid} resource={resource} skills={project.skills} send={send} />
                ))}
              </tbody>
            </table>
          </div>
        ))}
    </>
  );
}

function SkillItem({ skill, send }: { skill: SkillDef; send: Send }) {
  const s = useMessages(skillsTabMessages);
  return (
    <li>
      <input
        // An undone rename shows the engine's name again.
        key={skill.name}
        aria-label={s.nameOf(skill.name)}
        maxLength={MAX_NAME}
        defaultValue={skill.name}
        onBlur={async (event) => {
          const input = event.target;
          const name = input.value.trim();
          if (!name || name === skill.name) {
            input.value = skill.name;
            return;
          }
          // A refused name (taken, or with a comma) goes back; the footer says why.
          if (!(await send({ type: "rename_skill", id: skill.id, name }))) input.value = skill.name;
        }}
      />
      <Button
        Icon={X}
        className="btn-icon"
        aria-label={s.remove(skill.name)}
        title={s.removeTip}
        onClick={() => void send({ type: "delete_skill", id: skill.id })}
      />
    </li>
  );
}

/** One person's skills; quick clicks build on the list sent last. */
function SkillRow({ resource, skills, send }: { resource: Resource; skills: SkillDef[]; send: Send }) {
  const s = useMessages(skillsTabMessages);
  const { latest, apply } = usePendingList(resource.skills, (next) =>
    send({ type: "set_resource_skills", uid: resource.uid, skills: next }),
  );
  const toggle = (id: number) => {
    const current = latest();
    const next = current.includes(id) ? current.filter((own) => own !== id) : [...current, id].sort((a, b) => a - b);
    void apply(next);
  };
  return (
    <tr>
      <th scope="row">{resource.name}</th>
      {skills.map((skill) => (
        <td key={skill.id}>
          <input
            type="checkbox"
            aria-label={s.has(resource.name, skill.name)}
            checked={resource.skills.includes(skill.id)}
            onChange={() => toggle(skill.id)}
          />
        </td>
      ))}
    </tr>
  );
}
