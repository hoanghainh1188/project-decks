import { formatHours } from "../format";
import type { Project } from "../types";

export function TaskUsage({ project }: { project: Project }) {
  const dates = Array.from(
    new Set(project.usage.flatMap((row) => row.days.map((day) => day.date))),
  ).sort();
  const over = new Set(project.overallocations.map((item) => `${item.resource_uid}:${item.date}`));
  return (
    <div className="usage">
      <table>
        <thead>
          <tr>
            <th>Công việc / nguồn lực</th>
            {dates.map((date) => (
              <th key={date}>{date.slice(8)}/{date.slice(5, 7)}</th>
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
              <td className="empty">Chưa có công việc.</td>
            </tr>
          )}
        </tbody>
      </table>
    </div>
  );
}
