import type { ToolbarStates } from "../toolbarState";
import type { DepKind } from "../types";
import { MenuButton } from "./MenuButton";

export interface ToolbarActions {
  onUndo: () => void;
  onRedo: () => void;
  onAddTask: () => void;
  onAddMilestone: () => void;
  onAddRecurring: () => void;
  onIndent: () => void;
  onOutdent: () => void;
  onLink: (kind: DepKind) => void;
  onUnlink: () => void;
  onSplit: () => void;
  onDelete: () => void;
  onProjectInfo: () => void;
  onToggleEvm: () => void;
  onExportPdf: () => void;
}

interface Props extends ToolbarActions {
  states: ToolbarStates;
  evmOpen: boolean;
}

const LINKS: { kind: DepKind; label: string }[] = [
  { kind: "fs", label: "Kết thúc → bắt đầu (FS)" },
  { kind: "ss", label: "Bắt đầu → bắt đầu (SS)" },
  { kind: "ff", label: "Kết thúc → kết thúc (FF)" },
  { kind: "sf", label: "Bắt đầu → kết thúc (SF)" },
];

/**
 * Everyday commands in groups, left to right: history, creating tasks, editing the
 * selected task, the project, and reports. System commands and shortcuts stay in the
 * macOS menu. A command that cannot run is disabled with its reason as the tooltip.
 */
export function Toolbar({ states, evmOpen, ...actions }: Props) {
  const tip = (key: keyof ToolbarStates, fallback: string) => states[key].reason ?? fallback;
  return (
    <header className="toolbar">
      <strong className="toolbar-brand">ProjectDesk</strong>
      <div className="toolbar-group" role="group" aria-label="Lịch sử">
        <button type="button" disabled={!states.undo.enabled} title={tip("undo", "Hoàn tác (⌘Z)")} onClick={actions.onUndo}>
          Hoàn tác
        </button>
        <button
          type="button"
          disabled={!states.redo.enabled}
          title={tip("redo", "Làm lại (⇧⌘Z)")}
          onClick={actions.onRedo}
        >
          Làm lại
        </button>
      </div>
      <div className="toolbar-group" role="group" aria-label="Tạo công việc">
        <MenuButton
          label="Thêm việc"
          disabled={!states.add.enabled}
          title={tip("add", "Thêm sau công việc đang chọn")}
          items={[
            { label: "Công việc", onSelect: actions.onAddTask },
            { label: "Mốc", onSelect: actions.onAddMilestone },
            { label: "Công việc lặp…", onSelect: actions.onAddRecurring },
          ]}
        />
      </div>
      <div className="toolbar-group" role="group" aria-label="Công việc đang chọn">
        <button
          type="button"
          disabled={!states.outdent.enabled}
          title={tip("outdent", "Thụt ra (⌘[)")}
          onClick={actions.onOutdent}
        >
          Thụt ra
        </button>
        <button
          type="button"
          disabled={!states.indent.enabled}
          title={tip("indent", "Thụt vào (⌘])")}
          onClick={actions.onIndent}
        >
          Thụt vào
        </button>
        <MenuButton
          label="Liên kết"
          disabled={!states.link.enabled && !states.unlink.enabled}
          title={states.link.enabled || states.unlink.enabled ? "Liên kết với việc liền trước trong dàn ý" : tip("link", "")}
          items={[
            ...LINKS.map(({ kind, label }) => ({
              label,
              onSelect: () => actions.onLink(kind),
              disabled: !states.link.enabled,
              title: states.link.reason,
            })),
            {
              label: "Gỡ liên kết với việc liền trước",
              onSelect: actions.onUnlink,
              disabled: !states.unlink.enabled,
              title: states.unlink.reason,
            },
          ]}
        />
        <button
          type="button"
          disabled={!states.split.enabled}
          title={tip("split", "Ngắt công việc đang chọn thành nhiều đoạn")}
          onClick={actions.onSplit}
        >
          Tách…
        </button>
        <button
          type="button"
          className="danger"
          disabled={!states.remove.enabled}
          title={tip("remove", "Xóa công việc (⌘⌫)")}
          onClick={actions.onDelete}
        >
          Xóa
        </button>
      </div>
      <div className="toolbar-group" role="group" aria-label="Dự án">
        <button
          type="button"
          disabled={!states.project.enabled}
          title={tip("project", "Tên, ngày bắt đầu, ngày trạng thái và lịch dự án")}
          onClick={actions.onProjectInfo}
        >
          Dự án…
        </button>
      </div>
      <span className="spacer" />
      <div className="toolbar-group" role="group" aria-label="Báo cáo">
        <button
          type="button"
          className={evmOpen ? "on" : ""}
          aria-pressed={evmOpen}
          disabled={!states.evm.enabled}
          title={tip("evm", "Chỉ số EVM và burn-down")}
          onClick={actions.onToggleEvm}
        >
          EVM
        </button>
        <button
          type="button"
          disabled={!states.exportPdf.enabled}
          title={tip("exportPdf", "Xuất view hiện tại ra PDF")}
          onClick={actions.onExportPdf}
        >
          Xuất PDF
        </button>
      </div>
    </header>
  );
}
