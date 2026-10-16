import { ArrowLeft, Check, Download, Plug, Square, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import {
  errorMessage,
  onTrackerProgress,
  trackerCancel,
  trackerCheck,
  trackerForgetKey,
  trackerHasKey,
  trackerPull,
  trackerSaveKey,
  trackerTest,
} from "../../api";
import { useMessages } from "../../i18n";
import type { Command, Project, PullPreview, PullProgress, Snapshot } from "../../types";
import { Button } from "../Button";
import { Modal } from "../Modal";
import { trackerDialogMessages } from "./TrackerDialog.i18n";
import { counts, TrackerPreview } from "./TrackerPreview";
import { TrackerSetup } from "./TrackerSetup";
import { complete, formFor, sameSource, sourceOf, type SourceForm } from "./trackerSource";

type Stage = { kind: "setup" } | { kind: "fetching"; progress: PullProgress | null } | { kind: "preview"; preview: PullPreview };

/**
 * Tệp → Nhập → Kéo từ task tracker: name the tracker's site, project and filter, fetch the
 * issues, see what pulling them would add, change or mark gone (with problems by issue),
 * and apply it as one `apply_pull` (one undo step). The network and the API key stay in the
 * backend; this dialog only sends the key in once, to the credential store. Not a
 * transaction: nothing in the project changes before Áp dụng, which also saves the source
 * (its own undo step) when the form differs from the one the project holds.
 */
export function TrackerDialog({
  project,
  error,
  send,
  onClose,
}: {
  project: Project;
  /** The last command the engine refused. */
  error: string | null;
  send: (command: Command) => Promise<Snapshot | void>;
  onClose: () => void;
}) {
  const m = useMessages(trackerDialogMessages);
  const [form, setForm] = useState<SourceForm>(() => formFor(project.source));
  const [keyInput, setKeyInput] = useState("");
  const [hasKey, setHasKey] = useState(false);
  const [stage, setStage] = useState<Stage>({ kind: "setup" });
  const [note, setNote] = useState<{ text: string; bad: boolean } | null>(null);
  const [busy, setBusy] = useState(false);
  const sending = useRef(false);
  // The project the preview was made for: a later one means the changes must be asked again.
  const checkedFor = useRef<Project | null>(null);

  const source = sourceOf(form, project.source);
  const ready = complete(source);
  const fetching = stage.kind === "fetching";

  // Whether a key is saved for the site, for the one typed now.
  useEffect(() => {
    if (!source.site) {
      setHasKey(false);
      return;
    }
    let current = true;
    trackerHasKey(source.site)
      .then((has) => current && setHasKey(has))
      .catch(() => current && setHasKey(false));
    return () => {
      current = false;
    };
  }, [source.site]);

  // The preview follows the project: an email given to a person, or any edit under the dialog.
  const preview = stage.kind === "preview" ? stage.preview : null;
  useEffect(() => {
    if (!preview) return;
    if (checkedFor.current === project) return;
    checkedFor.current = project;
    let current = true;
    trackerCheck(preview.rows, preview.pulled_at)
      .then((changes) => current && setStage({ kind: "preview", preview: { ...preview, ...changes } }))
      .catch((err) => current && setNote({ text: errorMessage(err), bad: true }));
    return () => {
      current = false;
    };
  }, [project, preview]);

  async function saveTypedKey() {
    if (!keyInput.trim()) return;
    await trackerSaveKey(source.site, keyInput);
    setKeyInput("");
    setHasKey(true);
  }

  async function test() {
    setBusy(true);
    setNote({ text: m.testing, bad: false });
    try {
      await saveTypedKey();
      setNote({ text: m.testOk(await trackerTest(source)), bad: false });
    } catch (err) {
      setNote({ text: errorMessage(err), bad: true });
    } finally {
      setBusy(false);
    }
  }

  async function fetchIssues() {
    setNote(null);
    setStage({ kind: "fetching", progress: null });
    const stopListening = await onTrackerProgress((progress) =>
      setStage((current) => (current.kind === "fetching" ? { kind: "fetching", progress } : current)),
    );
    try {
      await saveTypedKey();
      const found = await trackerPull(source);
      checkedFor.current = project;
      setStage(found ? { kind: "preview", preview: found } : { kind: "setup" });
    } catch (err) {
      setNote({ text: errorMessage(err), bad: true });
      setStage({ kind: "setup" });
    } finally {
      stopListening();
    }
  }

  async function forgetKey() {
    try {
      await trackerForgetKey(source.site);
      setHasKey(false);
      setNote(null);
    } catch (err) {
      setNote({ text: errorMessage(err), bad: true });
    }
  }

  const summary = preview ? counts(preview) : null;
  const changeCount = summary ? summary.added + summary.changed + summary.gone : 0;
  const canApply = preview != null && preview.problems.length === 0 && changeCount > 0 && !busy;

  async function apply() {
    if (!preview || sending.current || !canApply) return;
    sending.current = true;
    setBusy(true);
    let done: Snapshot | void = undefined;
    if (!sameSource(project.source, source)) done = await send({ type: "set_source", source });
    else done = {} as Snapshot;
    if (done) done = await send({ type: "apply_pull", rows: preview.rows, pulled_at: preview.pulled_at });
    sending.current = false;
    setBusy(false);
    if (done) onClose();
  }

  const close = () => {
    if (sending.current) return;
    if (fetching) void trackerCancel();
    onClose();
  };

  const status = error ?? (note?.bad ? note.text : null);
  const foot =
    status != null ? (
      <span className="dialog-foot-note bad" role="alert">
        {status}
      </span>
    ) : (
      <span className="meta dialog-foot-note" role="status">
        {stage.kind === "fetching" ? m.progress(stage.progress?.fetched ?? 0, stage.progress?.total ?? null) : (note?.text ?? "")}
      </span>
    );

  const footer = (
    <>
      {foot}
      {stage.kind === "setup" && (
        <>
          <Button Icon={X} disabled={busy} onClick={close}>
            {m.cancel}
          </Button>
          <Button Icon={Plug} disabled={busy || !ready} onClick={() => void test()}>
            {m.test}
          </Button>
          <Button variant="primary" Icon={Download} disabled={busy || !ready} onClick={() => void fetchIssues()}>
            {m.fetch}
          </Button>
        </>
      )}
      {stage.kind === "fetching" && (
        <Button Icon={Square} onClick={() => void trackerCancel()}>
          {m.stopFetch}
        </Button>
      )}
      {stage.kind === "preview" && (
        <>
          <Button Icon={X} disabled={busy} onClick={close}>
            {m.cancel}
          </Button>
          <Button Icon={ArrowLeft} disabled={busy} onClick={() => setStage({ kind: "setup" })}>
            {m.back}
          </Button>
          <Button variant="primary" Icon={Check} disabled={!canApply} onClick={() => void apply()}>
            {m.apply(changeCount)}
          </Button>
        </>
      )}
    </>
  );

  return (
    <Modal title={m.title} className="wide tracker-dialog" onClose={close} footer={footer}>
      {stage.kind !== "preview" ? (
        <TrackerSetup
          form={form}
          onForm={setForm}
          keyInput={keyInput}
          onKeyInput={setKeyInput}
          hasKey={hasKey}
          onForgetKey={() => void forgetKey()}
          disabled={fetching || busy}
        />
      ) : (
        <TrackerPreview
          project={project}
          preview={stage.preview}
          onMatchEmail={(uid, email) => void send({ type: "set_resource_email", uid, email })}
        />
      )}
    </Modal>
  );
}
