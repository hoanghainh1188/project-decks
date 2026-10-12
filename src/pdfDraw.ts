import type { jsPDF } from "jspdf";

/** Drawing helpers shared by the Gantt and network PDFs. PDF export is always light. */

export interface Rgb {
  r: number;
  g: number;
  b: number;
}

/** A rectangle drawing is clipped to, in page millimetres. */
export interface Clip {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

/** `#rrggbb` as numbers. */
export function rgb(hex: string): Rgb {
  const value = hex.replace("#", "");
  return {
    r: Number.parseInt(value.slice(0, 2), 16),
    g: Number.parseInt(value.slice(2, 4), 16),
    b: Number.parseInt(value.slice(4, 6), 16),
  };
}

/** `fg` at `alpha` over `bg`: PDF has no transparency in the colours jsPDF sets. */
export function blend(fg: Rgb, bg: Rgb, alpha: number): Rgb {
  return {
    r: Math.round(fg.r * alpha + bg.r * (1 - alpha)),
    g: Math.round(fg.g * alpha + bg.g * (1 - alpha)),
    b: Math.round(fg.b * alpha + bg.b * (1 - alpha)),
  };
}

export function setText(doc: jsPDF, hex: string) {
  const color = rgb(hex);
  doc.setTextColor(color.r, color.g, color.b);
}

export function setFill(doc: jsPDF, hex: string) {
  const color = rgb(hex);
  doc.setFillColor(color.r, color.g, color.b);
}

export function setDraw(doc: jsPDF, hex: string) {
  const color = rgb(hex);
  doc.setDrawColor(color.r, color.g, color.b);
}

/** `text`, cut to fit `maxWidth` with "..." at the end, or "" when nothing fits. */
export function fitText(doc: Pick<jsPDF, "getTextWidth">, text: string, maxWidth: number): string {
  if (maxWidth <= 0) return "";
  if (doc.getTextWidth(text) <= maxWidth) return text;
  const chars = Array.from(text);
  let end = chars.length;
  while (end > 0 && doc.getTextWidth(`${chars.slice(0, end).join("")}...`) > maxWidth) end -= 1;
  return end === 0 ? "" : `${chars.slice(0, end).join("")}...`;
}

/** The part of a rectangle inside `clip`, or null when none is. */
export function clipRect(
  x: number,
  y: number,
  w: number,
  h: number,
  clip: Clip,
): { x: number; y: number; w: number; h: number } | null {
  const right = Math.min(x + w, clip.right);
  const bottom = Math.min(y + h, clip.bottom);
  const left = Math.max(x, clip.left);
  const top = Math.max(y, clip.top);
  if (right - left <= 0 || bottom - top <= 0) return null;
  return { x: left, y: top, w: right - left, h: bottom - top };
}

/** The part of a line segment inside `clip` (Cohen–Sutherland), or null when none is. */
export function clipSegment(
  x1: number,
  y1: number,
  x2: number,
  y2: number,
  clip: Clip,
): [number, number, number, number] | null {
  const leftBit = 1;
  const rightBit = 2;
  const topBit = 4;
  const bottomBit = 8;
  const code = (x: number, y: number) => {
    let bits = 0;
    if (x < clip.left) bits |= leftBit;
    else if (x > clip.right) bits |= rightBit;
    if (y < clip.top) bits |= topBit;
    else if (y > clip.bottom) bits |= bottomBit;
    return bits;
  };
  let c1 = code(x1, y1);
  let c2 = code(x2, y2);
  for (let guard = 0; guard < 12; guard += 1) {
    if ((c1 | c2) === 0) return [x1, y1, x2, y2];
    if ((c1 & c2) !== 0) return null;
    const out = c1 !== 0 ? c1 : c2;
    let x = x1;
    let y = y1;
    if (out & (bottomBit | topBit)) {
      if (y2 === y1) return null;
      const edge = out & bottomBit ? clip.bottom : clip.top;
      x = x1 + ((x2 - x1) * (edge - y1)) / (y2 - y1);
      y = edge;
    } else {
      if (x2 === x1) return null;
      const edge = out & rightBit ? clip.right : clip.left;
      y = y1 + ((y2 - y1) * (edge - x1)) / (x2 - x1);
      x = edge;
    }
    if (out === c1) {
      x1 = x;
      y1 = y;
      c1 = code(x1, y1);
    } else {
      x2 = x;
      y2 = y;
      c2 = code(x2, y2);
    }
  }
  return null;
}
