import { formatDuration, formatMoney, parseDuration } from "../../format";
import type { Accrual, Command, ConstraintType, Project, Task, TaskType } from "../../types";
import { DateTimeField } from "../DateTimeField";
import { AssignmentEditor } from "./AssignmentEditor";
import { canSplit, SplitSection } from "./SplitSection";
import { useInspectorOpen } from "./useInspectorOpen";

const CONSTRAINTS: { id: ConstraintType; label: string }[] = [
  { id: "asap", label: "Sớm nhất" },
  { id: "alap", label: "Muộn nhất" },
  { id: "snet", label: "Bắt đầu không sớm hơn" },
  { id: "snlt", label: "Bắt đầu không muộn hơn" },
  { id: "fnet", label: "Kết thúc không sớm hơn" },
  { id: "fnlt", label: "Kết thúc không muộn hơn" },
  { id: "mso", label: "Phải bắt đầu vào" },
  { id: "mfo", label: "Phải kết thúc vào" },
];

interface Props {
  project: Project;
  task: Task;
  send: (command: Command) => void;
  onEditSeries: (uid: number) => void;
  onSplit: (uid: number) => void;
  onEditCalendar: (calendarId: number) => void;
  onCreateCalendar: () => void;
}

/**
 * Properties of the selected task, grouped like Project's task information. Fields that
 * only make sense for a leaf (actuals, work, assignments, task type) are left out for a
 * summary task instead of being shown disabled.
 */
export function Inspector(props: Props) {
  const { task } = props;
  const [open, toggle] = useInspectorOpen();
  return (
    <section className="inspector" aria-label="Thuộc tính công việc">
      <header className="inspector-head">
        <button
          type="button"
          className="inspector-toggle"
          aria-expanded={open}
          aria-label="Chi tiết thuộc tính"
          title={open ? "Thu gọn" : "Mở rộng"}
          onClick={toggle}
        >
          {open ? "▾" : "▸"}
        </button>
        <h2>{task.name.trim() || "(Chưa đặt tên)"}</h2>
        <span className="meta">
          ID {task.id} · WBS {task.wbs}
          {task.summary ? " · Công việc tóm tắt" : ""}
          {task.milestone ? " · Mốc" : ""}
        </span>
        <span className="meta inspector-facts">
          Chi phí {formatMoney(task.cost)} · Thực tế {formatMoney(task.actual_cost)}
          {task.leveling_delay ? ` · Trễ cân bằng ${formatDuration(task.leveling_delay)}` : ""}
          {task.late ? " · Trễ hạn" : ""}
          {task.conflict ? " · Vi phạm ràng buộc" : ""}
        </span>
      </header>
      {open && (
        <div className="inspector-body">
          <SeriesGroup {...props} />
          <ScheduleGroup {...props} />
          {!task.summary && <TrackingGroup {...props} />}
          <CostGroup {...props} />
          {!task.summary && (
            <fieldset className="inspector-group assign-group">
              <legend>Nguồn lực</legend>
              <AssignmentEditor project={props.project} task={task} send={props.send} />
            </fieldset>
          )}
          <fieldset className="inspector-group notes-group">
            <legend>Ghi chú</legend>
            <input
              aria-label="Ghi chú"
              defaultValue={task.notes}
              key={`n-${task.uid}-${task.notes}`}
              onBlur={(event) => props.send({ type: "set_notes", uid: task.uid, notes: event.target.value })}
            />
          </fieldset>
        </div>
      )}
    </section>
  );
}

function SeriesGroup({ project, task, onEditSeries }: Props) {
  const series = task.series_uid != null ? project.tasks.find((item) => item.uid === task.series_uid) : null;
  if (!task.recurrence && !series) return null;
  return (
    <fieldset className="inspector-group">
      <legend>Công việc lặp</legend>
      <button type="button" onClick={() => onEditSeries(task.recurrence ? task.uid : (series?.uid ?? task.uid))}>
        Sửa cả chuỗi…
      </button>
      {series && (
        <p className="meta">
          {task.detached ? "Lần này đã sửa riêng; sinh lại chuỗi sẽ giữ nguyên." : `Một lần của chuỗi “${series.name}”.`}
        </p>
      )}
    </fieldset>
  );
}

function ScheduleGroup({ project, task, send, onEditCalendar, onCreateCalendar }: Props) {
  const needsDate = task.constraint_type !== "asap" && task.constraint_type !== "alap";
  return (
    <fieldset className="inspector-group">
      <legend>Lịch</legend>
      <div>
        <label>Ràng buộc</label>
        <select
          aria-label="Ràng buộc"
          value={task.constraint_type}
          onChange={(event) =>
            send({
              type: "set_constraint",
              uid: task.uid,
              constraint_type: event.target.value as ConstraintType,
              date: task.constraint_date,
            })
          }
        >
          {CONSTRAINTS.map((item) => (
            <option key={item.id} value={item.id}>{item.label}</option>
          ))}
        </select>
      </div>
      {needsDate && (
        <div>
          <label>Ngày ràng buộc</label>
          <DateTimeField
            ariaLabel="Ngày ràng buộc"
            value={task.constraint_date}
            onCommit={(date) => send({ type: "set_constraint", uid: task.uid, constraint_type: task.constraint_type, date })}
          />
        </div>
      )}
      <div>
        <label>Hạn chót</label>
        <DateTimeField
          optional
          ariaLabel="Hạn chót"
          value={task.deadline}
          onCommit={(date) => send({ type: "set_deadline", uid: task.uid, date })}
        />
      </div>
      {!task.summary && (
        <div>
          <label>Lịch công việc</label>
          <select
            aria-label="Lịch công việc"
            value={task.calendar_id ?? ""}
            onChange={(event) =>
              send({
                type: "set_task_calendar",
                uid: task.uid,
                calendar_id: event.target.value ? Number(event.target.value) : null,
              })
            }
          >
            <option value="">Lịch dự án</option>
            {project.calendars.map((calendar) => (
              <option key={calendar.id} value={calendar.id}>{calendar.name}</option>
            ))}
          </select>
          <div className="row">
            <button
              type="button"
              disabled={task.calendar_id == null}
              onClick={() => task.calendar_id != null && onEditCalendar(task.calendar_id)}
            >
              Sửa lịch
            </button>
            <button type="button" onClick={onCreateCalendar}>Tạo lịch riêng</button>
          </div>
        </div>
      )}
      {!task.summary && (
        <div>
          <label>Kiểu công việc</label>
          <select
            aria-label="Kiểu công việc"
            value={task.task_type}
            onChange={(event) =>
              send({
                type: "set_task_type",
                uid: task.uid,
                task_type: event.target.value as TaskType,
                effort_driven: task.effort_driven,
              })
            }
          >
            <option value="fixed_units">Đơn vị cố định</option>
            <option value="fixed_duration">Thời lượng cố định</option>
            <option value="fixed_work">Công việc cố định</option>
          </select>
          <label className="check">
            <input
              type="checkbox"
              checked={task.effort_driven}
              onChange={(event) =>
                send({
                  type: "set_task_type",
                  uid: task.uid,
                  task_type: task.task_type,
                  effort_driven: event.target.checked,
                })
              }
            />
            Hướng theo nỗ lực
          </label>
        </div>
      )}
      <label className="check">
        <input
          type="checkbox"
          checked={task.manual}
          onChange={(event) =>
            send({ type: "set_manual", uid: task.uid, manual: event.target.checked, start: task.start, finish: task.finish })
          }
        />
        Lên lịch tay
      </label>
      <div>
        <label>Ưu tiên</label>
        <input
          aria-label="Ưu tiên"
          className="narrow"
          defaultValue={task.priority}
          key={`p-${task.uid}-${task.priority}`}
          onBlur={(event) => send({ type: "set_priority", uid: task.uid, priority: Number(event.target.value) || 0 })}
        />
      </div>
    </fieldset>
  );
}

function TrackingGroup({ project, task, send, onSplit }: Props) {
  const overtime = project.assignments
    .filter((item) => item.task_uid === task.uid)
    .reduce((sum, item) => sum + item.overtime_work, 0);
  return (
    <fieldset className="inspector-group">
      <legend>Theo dõi</legend>
      <div>
        <label>Bắt đầu thực</label>
        <DateTimeField
          optional
          ariaLabel="Bắt đầu thực"
          value={task.actual_start}
          onCommit={(start) => send({ type: "set_actual_start", uid: task.uid, start })}
        />
      </div>
      <div>
        <label>Kết thúc thực</label>
        <DateTimeField
          optional
          ariaLabel="Kết thúc thực"
          value={task.actual_finish}
          onCommit={(finish) => send({ type: "set_actual_finish", uid: task.uid, finish })}
        />
      </div>
      <DurationInput
        label="Công việc"
        name="work"
        task={task}
        value={task.work}
        onCommit={(minutes) => send({ type: "set_work", uid: task.uid, minutes })}
      />
      <DurationInput
        label="Việc còn lại"
        name="rw"
        task={task}
        value={task.remaining_work}
        onCommit={(minutes) => send({ type: "set_remaining_work", uid: task.uid, minutes })}
      />
      <DurationInput
        label="Giờ làm thêm"
        name="ot"
        task={task}
        value={overtime}
        onCommit={(minutes) => send({ type: "set_overtime_work", uid: task.uid, minutes })}
      />
      {canSplit(task) && <SplitSection task={task} send={send} onSplit={onSplit} />}
    </fieldset>
  );
}

function DurationInput({
  label,
  name,
  task,
  value,
  onCommit,
}: {
  label: string;
  name: string;
  task: Task;
  value: number;
  onCommit: (minutes: number) => void;
}) {
  return (
    <div>
      <label>{label}</label>
      <input
        aria-label={label}
        className="narrow"
        defaultValue={formatDuration(value)}
        key={`${name}-${task.uid}-${value}`}
        onBlur={(event) => {
          const minutes = parseDuration(event.target.value);
          if (minutes !== null && minutes !== value) onCommit(minutes);
        }}
      />
    </div>
  );
}

function CostGroup({ task, send }: Props) {
  return (
    <fieldset className="inspector-group">
      <legend>Chi phí</legend>
      <div>
        <label>Chi phí cố định</label>
        <input
          aria-label="Chi phí cố định"
          className="narrow"
          defaultValue={task.fixed_cost}
          key={`c-${task.uid}-${task.fixed_cost}`}
          onBlur={(event) =>
            send({
              type: "set_fixed_cost",
              uid: task.uid,
              cost: Number(event.target.value) || 0,
              accrual: task.fixed_cost_accrual,
            })
          }
        />
      </div>
      <div>
        <label>Cộng dồn</label>
        <select
          aria-label="Cộng dồn"
          value={task.fixed_cost_accrual}
          onChange={(event) =>
            send({
              type: "set_fixed_cost",
              uid: task.uid,
              cost: task.fixed_cost,
              accrual: event.target.value as Accrual,
            })
          }
        >
          <option value="start">Đầu</option>
          <option value="prorated">Phân bổ</option>
          <option value="end">Cuối</option>
        </select>
      </div>
    </fieldset>
  );
}
