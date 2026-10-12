import { Flag, Trash2, X } from "lucide-react";
import { useState } from "react";

import { appMessages } from "../App.i18n";
import { useMessages } from "../i18n";
import { Button } from "./Button";
import { Modal } from "./Modal";

/** Set or clear baseline 0–10. */
export function BaselineDialog({
  onClose,
  onSet,
  onClear,
}: {
  onClose: () => void;
  onSet: (number: number) => void;
  onClear: (number: number) => void;
}) {
  const m = useMessages(appMessages);
  const [number, setNumber] = useState(0);
  return (
    <Modal
      title={m.baseline}
      onClose={onClose}
      footer={
        <>
          <Button variant="danger" Icon={Trash2} onClick={() => onClear(Math.min(10, Math.max(0, number)))}>{m.baselineDelete}</Button>
          <span className="spacer" />
          <Button Icon={X} onClick={onClose}>{m.close}</Button>
          <Button variant="primary" Icon={Flag} onClick={() => onSet(Math.min(10, Math.max(0, number)))}>{m.baselineSet}</Button>
        </>
      }
    >
      <label>{m.baselineNumber}</label>
      <input type="number" min={0} max={10} value={number} onChange={(event) => setNumber(Number(event.target.value))} />
    </Modal>
  );
}
