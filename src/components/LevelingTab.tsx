import { ArrowDown, ArrowDownWideNarrow, ArrowUp, ArrowUpNarrowWide, Plus, X } from "lucide-react";
import { useRef } from "react";

import { useMessages } from "../i18n";
import type { Command, Criterion, Project, Snapshot, SortKey } from "../types";
import { Button } from "./Button";
import { levelingTabMessages } from "./LevelingTab.i18n";

type Send = (command: Command) => Promise<Snapshot | void>;

/** The engine's limit (`level_order::MAX_SORT_KEYS`). */
const MAX_KEYS = 8;

const BUILT_IN = ["start", "priority", "slack", "deadline", "duration", "id"] as const;

const asc = (criterion: Criterion): SortKey => ({ criterion, descending: false });
const desc = (criterion: Criterion): SortKey => ({ criterion, descending: true });

/** What an empty list stands for, as the engine's `default_order`. */
const STANDARD: SortKey[] = [asc({ type: "start" }), desc({ type: "priority" }), asc({ type: "slack" })];

/** Microsoft Project's three orders: Standard, Priority/Standard, ID Only. */
const PRESETS: { id: "standard" | "priorityFirst" | "byId"; order: SortKey[] }[] = [
  { id: "standard", order: [] },
  { id: "priorityFirst", order: [desc({ type: "priority" }), asc({ type: "start" }), asc({ type: "slack" })] },
  { id: "byId", order: [asc({ type: "id" })] },
];

/** A criterion as a select value: the built-in name, or `field:<id>`. */
function criterionKey(criterion: Criterion): string {
  return criterion.type === "field" ? `field:${criterion.id}` : criterion.type;
}

function criterionOf(key: string): Criterion {
  if (key.startsWith("field:")) return { type: "field", id: Number(key.slice("field:".length)) };
  return { type: key as (typeof BUILT_IN)[number] };
}

function sameOrder(a: readonly SortKey[], b: readonly SortKey[]): boolean {
  return (
    a.length === b.length &&
    a.every((key, index) => criterionKey(key.criterion) === criterionKey(b[index].criterion) && key.descending === b[index].descending)
  );
}

/**
 * The order leveling places tasks in, inside Thông tin dự án. Every change sends the whole
 * list at once; Hủy undoes them with the rest of the dialog. An empty list is the
 * standard order, shown as its three criteria; editing one of them sets the list.
 */
export function LevelingTab({ project, send }: { project: Project; send: Send }) {
  const l = useMessages(levelingTabMessages);
  const stored = project.leveling_order;
  const order = stored.length > 0 ? stored : STANDARD;
  // The list the next change starts from: the last one sent. A new snapshot replaces it
  // only once no change is still on its way, so two quick clicks both count.
  const latest = useRef(order);
  const seen = useRef(stored);
  const pending = useRef(0);
  if (seen.current !== stored) {
    seen.current = stored;
    if (pending.current === 0) latest.current = order;
  }
  // The criterion whose select gets focus once its row comes back with a new key.
  const focusKey = useRef<string | null>(null);
  const options = [
    ...BUILT_IN.map((type) => ({ key: type as string, name: l[type] })),
    ...project.fields.map((field) => ({ key: `field:${field.id}`, name: field.name })),
  ];
  const nameOf = (criterion: Criterion) =>
    options.find((option) => option.key === criterionKey(criterion))?.name ?? criterionKey(criterion);
  const used = new Set(order.map((key) => criterionKey(key.criterion)));
  const full = order.length >= MAX_KEYS;
  const standard = sameOrder(order, STANDARD);

  async function apply(next: SortKey[]) {
    latest.current = next;
    pending.current += 1;
    try {
      // Refused: start again from the list the engine holds (its reason is in the footer).
      if (!(await send({ type: "set_leveling_order", order: next }))) {
        latest.current = seen.current.length > 0 ? seen.current : STANDARD;
      }
    } finally {
      pending.current -= 1;
    }
  }
  const at = (own: string) => latest.current.findIndex((item) => criterionKey(item.criterion) === own);
  const replace = (own: string, key: SortKey) => {
    if (at(own) < 0) return;
    if (criterionKey(key.criterion) !== own) focusKey.current = criterionKey(key.criterion);
    void apply(latest.current.map((item) => (criterionKey(item.criterion) === own ? key : item)));
  };
  const move = (own: string, by: -1 | 1) => {
    const index = at(own);
    const other = index + by;
    if (index < 0 || other < 0 || other >= latest.current.length) return;
    const next = [...latest.current];
    [next[index], next[other]] = [next[other], next[index]];
    void apply(next);
  };
  const remove = (own: string) => {
    if (latest.current.length > 1) void apply(latest.current.filter((item) => criterionKey(item.criterion) !== own));
  };
  const add = () => {
    const taken = new Set(latest.current.map((item) => criterionKey(item.criterion)));
    const free = options.find((option) => !taken.has(option.key));
    if (free && latest.current.length < MAX_KEYS) void apply([...latest.current, asc(criterionOf(free.key))]);
  };

  return (
    <>
      <p className="meta">{l.intro}</p>
      <div className="row" role="group" aria-label={l.presets}>
        {PRESETS.map((preset) => (
          <Button
            key={preset.id}
            aria-pressed={preset.order.length > 0 ? sameOrder(order, preset.order) : standard}
            title={l[`${preset.id}Tip`]}
            onClick={() => void apply(preset.order)}
          >
            {l[preset.id]}
          </Button>
        ))}
      </div>
      {standard && <p className="meta">{l.isStandard}</p>}
      {/* Without list-style, Safari drops the list role unless it is given. */}
      <ol className="sort-keys" role="list" aria-label={l.order}>
        {order.map((key, index) => {
          const own = criterionKey(key.criterion);
          return (
            <SortKeyRow
              key={own}
              index={index}
              count={order.length}
              sortKey={key}
              name={nameOf(key.criterion)}
              options={options}
              used={used}
              selectRef={(element) => {
                if (element && focusKey.current === own) {
                  focusKey.current = null;
                  element.focus();
                }
              }}
              onChange={(next) => replace(own, next)}
              onMove={(by) => move(own, by)}
              onRemove={() => remove(own)}
            />
          );
        })}
      </ol>
      <div className="row">
        <Button Icon={Plus} disabled={full || used.size === options.length} onClick={add}>
          {l.add}
        </Button>
        <span className="meta">{l.limit(MAX_KEYS)}</span>
      </div>
      <p className="meta">{l.note}</p>
    </>
  );
}

function SortKeyRow({
  index,
  count,
  sortKey,
  name,
  options,
  used,
  selectRef,
  onChange,
  onMove,
  onRemove,
}: {
  index: number;
  count: number;
  sortKey: SortKey;
  name: string;
  options: { key: string; name: string }[];
  used: Set<string>;
  selectRef: (element: HTMLSelectElement | null) => void;
  onChange: (key: SortKey) => void;
  onMove: (by: -1 | 1) => void;
  onRemove: () => void;
}) {
  const l = useMessages(levelingTabMessages);
  const own = criterionKey(sortKey.criterion);
  const DirectionIcon = sortKey.descending ? ArrowDownWideNarrow : ArrowUpNarrowWide;
  return (
    <li>
      <span className="sort-rank" aria-hidden="true">
        {index + 1}
      </span>
      <select
        ref={selectRef}
        aria-label={l.rank(index + 1)}
        value={own}
        onChange={(event) => onChange({ ...sortKey, criterion: criterionOf(event.target.value) })}
      >
        {options.map((option) => (
          <option key={option.key} value={option.key} disabled={option.key !== own && used.has(option.key)}>
            {option.name}
          </option>
        ))}
      </select>
      <Button
        Icon={DirectionIcon}
        aria-label={l.directionOf(sortKey.descending ? l.descending : l.ascending, name)}
        title={l.direction}
        onClick={() => onChange({ ...sortKey, descending: !sortKey.descending })}
      >
        {sortKey.descending ? l.descending : l.ascending}
      </Button>
      <Button Icon={ArrowUp} className="btn-icon" aria-label={l.moveUp(name)} disabled={index === 0} onClick={() => onMove(-1)} />
      <Button
        Icon={ArrowDown}
        className="btn-icon"
        aria-label={l.moveDown(name)}
        disabled={index === count - 1}
        onClick={() => onMove(1)}
      />
      <Button
        Icon={X}
        className="btn-icon"
        aria-label={l.remove(name)}
        title={l.removeTip}
        disabled={count === 1}
        onClick={onRemove}
      />
    </li>
  );
}
