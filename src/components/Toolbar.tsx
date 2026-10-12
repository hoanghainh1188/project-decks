import {
  ChartLine,
  Diamond,
  FileDown,
  FolderCog,
  IndentDecrease,
  IndentIncrease,
  Link2,
  ListPlus,
  Plus,
  Redo2,
  Repeat,
  Scissors,
  Trash2,
  Undo2,
  Unlink2,
  type LucideIcon,
} from "lucide-react";
import type { ButtonHTMLAttributes } from "react";

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

/**
 * Each group has its own colour, so the eye finds a command by where it belongs:
 * history gray, creating green, editing the selected task blue, deleting red, the
 * project purple, reports amber.
 */
type Tone = "history" | "create" | "edit" | "danger" | "project" | "report";

const LINKS: { kind: DepKind; label: string }[] = [
  { kind: "fs", label: "Kết thúc → bắt đầu (FS)" },
  { kind: "ss", label: "Bắt đầu → bắt đầu (SS)" },
  { kind: "ff", label: "Kết thúc → kết thúc (FF)" },
  { kind: "sf", label: "Bắt đầu → kết thúc (SF)" },
];

function icon(Icon: LucideIcon) {
  return <Icon aria-hidden="true" size={15} strokeWidth={1.9} />;
}

interface ToolProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  tone: Tone;
  Icon: LucideIcon;
  label: string;
}

function Tool({ tone, Icon, label, className, ...rest }: ToolProps) {
  return (
    <button type="button" className={["tool", `tone-${tone}`, className].filter(Boolean).join(" ")} {...rest}>
      {icon(Icon)}
      <span>{label}</span>
    </button>
  );
}

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
        <Tool
          tone="history"
          Icon={Undo2}
          label="Hoàn tác"
          disabled={!states.undo.enabled}
          title={tip("undo", "Hoàn tác (⌘Z)")}
          onClick={actions.onUndo}
        />
        <Tool
          tone="history"
          Icon={Redo2}
          label="Làm lại"
          disabled={!states.redo.enabled}
          title={tip("redo", "Làm lại (⇧⌘Z)")}
          onClick={actions.onRedo}
        />
      </div>
      <div className="toolbar-group" role="group" aria-label="Tạo công việc">
        <MenuButton
          label="Thêm việc"
          className="tool tone-create primary"
          icon={icon(Plus)}
          disabled={!states.add.enabled}
          title={tip("add", "Thêm sau công việc đang chọn")}
          items={[
            { label: "Công việc", icon: icon(ListPlus), onSelect: actions.onAddTask },
            { label: "Mốc", icon: icon(Diamond), onSelect: actions.onAddMilestone },
            { label: "Công việc lặp…", icon: icon(Repeat), onSelect: actions.onAddRecurring },
          ]}
        />
      </div>
      <div className="toolbar-group" role="group" aria-label="Công việc đang chọn">
        <Tool
          tone="edit"
          Icon={IndentDecrease}
          label="Thụt ra"
          disabled={!states.outdent.enabled}
          title={tip("outdent", "Thụt ra (⌘[)")}
          onClick={actions.onOutdent}
        />
        <Tool
          tone="edit"
          Icon={IndentIncrease}
          label="Thụt vào"
          disabled={!states.indent.enabled}
          title={tip("indent", "Thụt vào (⌘])")}
          onClick={actions.onIndent}
        />
        <MenuButton
          label="Liên kết"
          className="tool tone-edit"
          icon={icon(Link2)}
          disabled={!states.link.enabled && !states.unlink.enabled}
          title={states.link.enabled || states.unlink.enabled ? "Liên kết với việc liền trước trong dàn ý" : tip("link", "")}
          items={[
            ...LINKS.map(({ kind, label }) => ({
              label,
              icon: icon(Link2),
              onSelect: () => actions.onLink(kind),
              disabled: !states.link.enabled,
              title: states.link.reason,
            })),
            {
              label: "Gỡ liên kết với việc liền trước",
              icon: icon(Unlink2),
              onSelect: actions.onUnlink,
              disabled: !states.unlink.enabled,
              title: states.unlink.reason,
            },
          ]}
        />
        <Tool
          tone="edit"
          Icon={Scissors}
          label="Tách…"
          disabled={!states.split.enabled}
          title={tip("split", "Ngắt công việc đang chọn thành nhiều đoạn")}
          onClick={actions.onSplit}
        />
        <Tool
          tone="danger"
          Icon={Trash2}
          label="Xóa"
          disabled={!states.remove.enabled}
          title={tip("remove", "Xóa công việc (⌘⌫)")}
          onClick={actions.onDelete}
        />
      </div>
      <div className="toolbar-group" role="group" aria-label="Dự án">
        <Tool
          tone="project"
          Icon={FolderCog}
          label="Dự án…"
          disabled={!states.project.enabled}
          title={tip("project", "Tên, ngày bắt đầu, ngày trạng thái và lịch dự án")}
          onClick={actions.onProjectInfo}
        />
      </div>
      <span className="spacer" />
      <div className="toolbar-group" role="group" aria-label="Báo cáo">
        <Tool
          tone="report"
          Icon={ChartLine}
          label="EVM"
          className={evmOpen ? "on" : undefined}
          aria-pressed={evmOpen}
          disabled={!states.evm.enabled}
          title={tip("evm", "Chỉ số EVM và burn-down")}
          onClick={actions.onToggleEvm}
        />
        <Tool
          tone="report"
          Icon={FileDown}
          label="Xuất PDF"
          disabled={!states.exportPdf.enabled}
          title={tip("exportPdf", "Xuất view hiện tại ra PDF")}
          onClick={actions.onExportPdf}
        />
      </div>
    </header>
  );
}
