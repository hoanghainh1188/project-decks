import { formatHours } from "../format";
import { useMessages } from "../i18n";
import type { Project } from "../types";
import { taskUsageMessages } from "./TaskUsage.i18n";

export function TaskUsage({ project }: { project: Project }) {
  const m = useMessages(taskUsageMessages);
  const dates = Array.from(
    new Set(project.usage.flatMap((row) => row.days.map((day) => day.date))),
  ).sort();
  const over = new Set(project.overallocations.map((item) => `${item.resource_uid}:${item.date}`));
  return (
    <div className="usage">
      <table>
        <thead>
          <tr>
            <th>{m.head}</th>
            {dates.map((date) => (
              <th key={date}>{m.day(date)}</th>
            ))}
          </tr>
        </thead>
        <tbody>
          {project.usage.map((row) => (
            <tr key={`${row.task_uid}-${row.assignment_uid ?? "t"}`} className={row.assignment_uid ? "child" : ""}>
              <td>{row.label}</td>
              {dates.map((date) => {
                const hours = row.days.find((day) => day.date === date)?.hours ?? 0;
                // Only cells with work show the overload; the other tasks of that person are not to blame.
                const text = formatHours(hours);
                const hot = text !== "" && row.resource_uid !== null && over.has(`${row.resource_uid}:${date}`);
                return (
                  <td key={date} className={hot ? "over" : ""}>
                    {text}
                  </td>
                );
              })}
            </tr>
          ))}
          {project.usage.length === 0 && (
            <tr>
              <td className="empty">{m.empty}</td>
            </tr>
          )}
        </tbody>
      </table>
    </div>
  );
}
