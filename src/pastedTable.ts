import type { ImportSheet } from "./types";

/** The same limits `pick_table` applies to a file. */
export const MAX_ROWS = 20_000;
export const MAX_COLUMNS = 100;

/**
 * Cells copied from a spreadsheet (Excel, Numbers, Google Sheets put tab-separated text on
 * the clipboard): rows by line, cells by tab, a cell in double quotes may hold tabs, line
 * breaks and doubled quotes. Null when the text does not look like a table: no tab and
 * fewer than two lines, which is a name or a sentence rather than rows to import.
 */
export function parsePastedTable(text: string): ImportSheet | null {
  const body = text.replace(/\r\n?/g, "\n").replace(/\n+$/, "");
  if (!body.includes("\t") && !body.includes("\n")) return null;
  const rows: string[][] = [];
  const lines: number[] = [];
  let line = 1;
  let rowLine = 1;
  let cells: string[] = [];
  let cell = "";
  let quoted = false;
  let atCellStart = true;
  const endRow = () => {
    cells.push(cell);
    if (cells.some((value) => value.trim() !== "") && rows.length < MAX_ROWS) {
      rows.push(cells.slice(0, MAX_COLUMNS).map((value) => value.trim()));
      lines.push(rowLine);
    }
    cells = [];
    cell = "";
    atCellStart = true;
  };
  for (let index = 0; index < body.length; index += 1) {
    const char = body[index];
    if (quoted) {
      if (char === '"' && body[index + 1] === '"') {
        cell += '"';
        index += 1;
      } else if (char === '"') quoted = false;
      else {
        if (char === "\n") line += 1;
        cell += char;
      }
      continue;
    }
    if (char === '"' && atCellStart) {
      quoted = true;
      atCellStart = false;
    } else if (char === "\t") {
      cells.push(cell);
      cell = "";
      atCellStart = true;
    } else if (char === "\n") {
      endRow();
      line += 1;
      rowLine = line;
    } else {
      cell += char;
      atCellStart = false;
    }
  }
  endRow();
  if (rows.length === 0) return null;
  const width = Math.max(...rows.map((row) => row.length));
  return { sheets: [], sheet: null, rows: rows.map((row) => [...row, ...Array(width - row.length).fill("")]), lines };
}
