import { Check, X } from "lucide-react";
import { useCallback, useRef, useState, type KeyboardEvent } from "react";

import { useMessages } from "../i18n";
import { commitFocusedField } from "../shortcuts";
import type { Command, Project, Snapshot } from "../types";
import { assignmentDialogMessages } from "./AssignmentDialog.i18n";
import { Button } from "./Button";
import { Modal } from "./Modal";
import { RulesTab } from "./RulesTab";
import { SkillsTab } from "./SkillsTab";

export type AssignmentTab = "skills" | "rules";
const TABS: AssignmentTab[] = ["skills", "rules"];

/**
 * Skills and assignment rules. A transaction like Thông tin dự án: edits apply as they
 * are made, Lưu keeps them and Hủy (✕, Escape) undoes back to where the dialog opened.
 */
export function AssignmentDialog({
  project,
  changed,
  error,
  onSave,
  onCancel,
  send,
  initialTab = "rules",
}: {
  project: Project;
  changed: boolean;
  /** The last edit the engine refused. */
  error: string | null;
  onSave: () => void;
  onCancel: () => void;
  send: (command: Command) => Promise<Snapshot | void>;
  initialTab?: AssignmentTab;
}) {
  const a = useMessages(assignmentDialogMessages);
  const [tab, setTab] = useState<AssignmentTab>(initialTab);
  const tabRefs = useRef<Record<AssignmentTab, HTMLButtonElement | null>>({ skills: null, rules: null });
  // Edits still on their way, and whether the last one answered was refused.
  const inFlight = useRef(new Set<Promise<Snapshot | void>>());
  const refused = useRef(false);
  const tracked = useCallback(
    (command: Command) => {
      const sent = send(command).then((state) => {
        refused.current = !state;
        return state;
      });
      inFlight.current.add(sent);
      void sent.finally(() => inFlight.current.delete(sent));
      return sent;
    },
    [send],
  );

  // Lưu leaves the field being typed in first, and closes only once the engine answered
  // every edit; when the last one was refused the dialog stays, the reason in the footer.
  async function save() {
    commitFocusedField();
    while (inFlight.current.size > 0) await Promise.all([...inFlight.current]);
    const ok = !refused.current;
    refused.current = false;
    if (ok) onSave();
  }

  function onTabKey(event: KeyboardEvent) {
    const index = TABS.indexOf(tab);
    const next =
      event.key === "ArrowRight" ? TABS[(index + 1) % TABS.length]
      : event.key === "ArrowLeft" ? TABS[(index + TABS.length - 1) % TABS.length]
      : event.key === "Home" ? TABS[0]
      : event.key === "End" ? TABS[TABS.length - 1]
      : null;
    if (!next) return;
    event.preventDefault();
    setTab(next);
    tabRefs.current[next]?.focus();
  }

  return (
    <Modal
      title={a.title}
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
      <div className="view-tabs dialog-tabs" role="tablist" aria-label={a.tabs} onKeyDown={onTabKey}>
        {TABS.map((id) => (
          <button
            key={id}
            ref={(element) => {
              tabRefs.current[id] = element;
            }}
            type="button"
            role="tab"
            id={`assignment-tab-${id}`}
            aria-selected={tab === id}
            aria-controls="assignment-panel"
            tabIndex={tab === id ? 0 : -1}
            className={`view-tab${tab === id ? " on" : ""}`}
            onClick={() => setTab(id)}
          >
            {a[id]}
          </button>
        ))}
      </div>
      <div
        className="dialog-tabpanel"
        role="tabpanel"
        id="assignment-panel"
        aria-labelledby={`assignment-tab-${tab}`}
        tabIndex={0}
      >
        {tab === "skills" ? <SkillsTab project={project} send={tracked} /> : <RulesTab project={project} send={tracked} />}
      </div>
    </Modal>
  );
}
