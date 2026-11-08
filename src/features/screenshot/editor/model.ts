export interface Point {
  x: number;
  y: number;
}

export interface Rect extends Point {
  width: number;
  height: number;
}

export type AnnotationTool =
  | 'rect'
  | 'ellipse'
  | 'line'
  | 'arrow'
  | 'freehand'
  | 'text'
  | 'bubble'
  | 'step'
  | 'image'
  | 'emoji'
  | 'highlight'
  | 'blur'
  | 'pixelate'
  | 'magnifier'
  | 'spotlight'
  | 'cutout';

export type Tool = AnnotationTool | 'select' | 'crop' | 'eraser';
export type CutoutDirection = 'horizontal' | 'vertical';

export interface AnnotationStyle {
  color: string;
  fill: boolean;
  strokeWidth: number;
  opacity: number;
  fontFamily: string;
  fontSize: number;
  amount: number;
  cutoutDirection?: CutoutDirection;
}

export const defaultAnnotationStyle: Readonly<AnnotationStyle> = {
  color: '#facc15',
  fill: false,
  strokeWidth: 4,
  opacity: 1,
  fontFamily: 'Segoe UI',
  fontSize: 28,
  amount: 12,
};

export interface Annotation {
  id: string;
  tool: AnnotationTool;
  box: Rect;
  style: AnnotationStyle;
  points?: readonly Point[];
  text?: string;
  assetId?: string;
}

export interface AnnotationDocument {
  width: number;
  height: number;
  crop: Rect | null;
  annotations: readonly Annotation[];
}

export interface History {
  past: readonly AnnotationDocument[];
  present: AnnotationDocument;
  future: readonly AnnotationDocument[];
}

const HISTORY_LIMIT = 64;
const defaultText: Partial<Record<AnnotationTool, string>> = {
  text: 'Văn bản',
  bubble: 'Ghi chú',
  step: '1',
  emoji: '🙂',
};

const clamp = (value: number, lower: number, upper: number) =>
  Math.max(lower, Math.min(upper, value));

export function createDocument(width: number, height: number): AnnotationDocument {
  if (!Number.isSafeInteger(width) || !Number.isSafeInteger(height) || width < 1 || height < 1) {
    throw new Error('Kích thước ảnh chụp không hợp lệ.');
  }
  return { width, height, crop: null, annotations: [] };
}

export function boxFromPoints(start: Point, end: Point): Rect {
  return {
    x: Math.min(start.x, end.x),
    y: Math.min(start.y, end.y),
    width: Math.abs(start.x - end.x),
    height: Math.abs(start.y - end.y),
  };
}

export function clampCrop(rect: Rect, width: number, height: number): Rect {
  if (![rect.x, rect.y, rect.width, rect.height].every(Number.isFinite)) {
    throw new Error('Vùng chụp không hợp lệ.');
  }
  const x = clamp(Math.floor(rect.x), 0, width - 1);
  const y = clamp(Math.floor(rect.y), 0, height - 1);
  const right = clamp(Math.ceil(rect.x + rect.width), x + 1, width);
  const bottom = clamp(Math.ceil(rect.y + rect.height), y + 1, height);
  return { x, y, width: right - x, height: bottom - y };
}

export function updateCrop(document: AnnotationDocument, crop: Rect | null): AnnotationDocument {
  const next = crop ? clampCrop(crop, document.width, document.height) : null;
  const previous = document.crop;
  if (
    next === previous ||
    (next &&
      previous &&
      next.x === previous.x &&
      next.y === previous.y &&
      next.width === previous.width &&
      next.height === previous.height)
  ) {
    return document;
  }
  return { ...document, crop: next };
}

export function moveCrop(document: AnnotationDocument, dx: number, dy: number): AnnotationDocument {
  const crop = document.crop;
  if (!crop) return document;
  return updateCrop(document, {
    ...crop,
    x: clamp(Math.round(crop.x + dx), 0, document.width - crop.width),
    y: clamp(Math.round(crop.y + dy), 0, document.height - crop.height),
  });
}

export function containsPoint(box: Rect, point: Point, tolerance = 0): boolean {
  return (
    point.x >= box.x - tolerance &&
    point.x <= box.x + box.width + tolerance &&
    point.y >= box.y - tolerance &&
    point.y <= box.y + box.height + tolerance
  );
}

interface Segment {
  start: number;
  length: number;
  offset: number;
}

interface OutputLayout {
  crop: Rect;
  columns: readonly Segment[];
  rows: readonly Segment[];
  width: number;
  height: number;
}

function sourceCrop(document: AnnotationDocument): Rect {
  return document.crop
    ? clampCrop(document.crop, document.width, document.height)
    : { x: 0, y: 0, width: document.width, height: document.height };
}

function cutoutBounds(annotation: Annotation, crop: Rect): Rect | null {
  const box = annotation.box;
  if (annotation.style.cutoutDirection === 'vertical') {
    const left = Math.max(crop.x, Math.floor(box.x));
    const right = Math.min(crop.x + crop.width, Math.ceil(box.x + box.width));
    return right > left ? { x: left, y: crop.y, width: right - left, height: crop.height } : null;
  }
  const top = Math.max(crop.y, Math.floor(box.y));
  const bottom = Math.min(crop.y + crop.height, Math.ceil(box.y + box.height));
  return bottom > top ? { x: crop.x, y: top, width: crop.width, height: bottom - top } : null;
}

export function cutoutGuide(document: AnnotationDocument, annotation: Annotation): Rect | null {
  return cutoutBounds(annotation, sourceCrop(document));
}

function retainedSegments(start: number, end: number, cuts: [number, number][]) {
  const spans: Segment[] = [];
  let cursor = start;
  let size = 0;
  for (const [left, right] of cuts.sort((a, b) => a[0] - b[0])) {
    if (right <= cursor) continue;
    if (left > cursor) {
      const length = left - cursor;
      spans.push({ start: cursor, length, offset: size });
      size += length;
    }
    cursor = right;
  }
  if (cursor < end) {
    const length = end - cursor;
    spans.push({ start: cursor, length, offset: size });
    size += length;
  }
  return { spans, size };
}

export function outputLayout(document: AnnotationDocument): OutputLayout {
  const crop = sourceCrop(document);
  const columnCuts: [number, number][] = [];
  const rowCuts: [number, number][] = [];
  for (const annotation of document.annotations) {
    if (annotation.tool !== 'cutout') continue;
    const cut = cutoutBounds(annotation, crop);
    if (!cut) continue;
    if (annotation.style.cutoutDirection === 'vertical') {
      columnCuts.push([cut.x, cut.x + cut.width]);
    } else {
      rowCuts.push([cut.y, cut.y + cut.height]);
    }
  }
  const columns = retainedSegments(crop.x, crop.x + crop.width, columnCuts);
  const rows = retainedSegments(crop.y, crop.y + crop.height, rowCuts);
  return {
    crop,
    columns: columns.spans,
    rows: rows.spans,
    width: columns.size,
    height: rows.size,
  };
}

export function outputSize(document: AnnotationDocument): { width: number; height: number } {
  const { width, height } = outputLayout(document);
  return { width, height };
}

function boundsOfPoints(points: readonly Point[], fallback: Rect): Rect {
  const first = points[0];
  if (!first) return fallback;
  let left = first.x;
  let top = first.y;
  let right = first.x;
  let bottom = first.y;
  for (const point of points) {
    left = Math.min(left, point.x);
    top = Math.min(top, point.y);
    right = Math.max(right, point.x);
    bottom = Math.max(bottom, point.y);
  }
  return { x: left, y: top, width: right - left, height: bottom - top };
}

export function createAnnotation(
  id: string,
  tool: AnnotationTool,
  start: Point,
  end: Point,
  style: AnnotationStyle = defaultAnnotationStyle,
  extras: Pick<Partial<Annotation>, 'points' | 'text' | 'assetId'> = {},
): Annotation {
  const points =
    tool === 'line' || tool === 'arrow' || tool === 'freehand'
      ? (extras.points ?? [start, end]).map(({ x, y }) => ({ x, y }))
      : undefined;
  const dragBox = boxFromPoints(start, end);
  const text = extras.text ?? defaultText[tool];
  let box = points ? boundsOfPoints(points, dragBox) : dragBox;

  if (text !== undefined && box.width < 4 && box.height < 4) {
    const size = Math.max(8, style.fontSize);
    const diameter = Math.max(20, Math.round(size * 1.6));
    box =
      tool === 'step' || tool === 'emoji'
        ? { x: start.x, y: start.y, width: diameter, height: diameter }
        : {
            x: start.x,
            y: start.y,
            width: Math.max(size * 5, Array.from(text).length * size * 0.65),
            height: Math.ceil(size * (tool === 'bubble' ? 2.5 : 1.6)),
          };
  }
  if (tool === 'image' && box.width < 4 && box.height < 4) {
    box = { x: start.x, y: start.y, width: 192, height: 144 };
  }
  return {
    id,
    tool,
    box: { ...box, width: Math.max(1, box.width), height: Math.max(1, box.height) },
    style: { ...style },
    points,
    text,
    assetId: extras.assetId,
  };
}

export function moveAnnotation(annotation: Annotation, dx: number, dy: number): Annotation {
  if (dx === 0 && dy === 0) return annotation;
  const { box, points } = annotation;
  return {
    ...annotation,
    box: { ...box, x: box.x + dx, y: box.y + dy },
    points: points?.map(({ x, y }) => ({ x: x + dx, y: y + dy })),
  };
}

export function resizeAnnotation(annotation: Annotation, box: Rect): Annotation {
  const old = annotation.box;
  const next = { ...box, width: Math.max(1, box.width), height: Math.max(1, box.height) };
  return {
    ...annotation,
    box: next,
    points: annotation.points?.map(({ x, y }) => ({
      x: next.x + ((x - old.x) * next.width) / old.width,
      y: next.y + ((y - old.y) * next.height) / old.height,
    })),
  };
}

function distanceToSegment(point: Point, start: Point, end: Point): number {
  const dx = end.x - start.x;
  const dy = end.y - start.y;
  const lengthSquared = dx * dx + dy * dy;
  const position =
    lengthSquared === 0
      ? 0
      : clamp(((point.x - start.x) * dx + (point.y - start.y) * dy) / lengthSquared, 0, 1);
  return Math.hypot(point.x - start.x - position * dx, point.y - start.y - position * dy);
}

export function hitTest(
  document: AnnotationDocument,
  point: Point,
  tolerance = 4,
): Annotation | null {
  for (let index = document.annotations.length - 1; index >= 0; index--) {
    const annotation = document.annotations[index];
    if (!annotation) continue;
    const range = Math.max(0, tolerance) + annotation.style.strokeWidth / 2;
    const box = annotation.tool === 'cutout' ? cutoutGuide(document, annotation) : annotation.box;
    if (!box || !containsPoint(box, point, range)) continue;

    const points = annotation.points;
    if (!points?.length) return annotation;
    const first = points[0];
    if (first && Math.hypot(point.x - first.x, point.y - first.y) <= range) return annotation;
    for (let segment = 1; segment < points.length; segment++) {
      const start = points[segment - 1];
      const end = points[segment];
      if (start && end && distanceToSegment(point, start, end) <= range) return annotation;
    }
  }
  return null;
}

export function createHistory(document: AnnotationDocument): History {
  return { past: [], present: document, future: [] };
}

export function commit(history: History, document: AnnotationDocument): History {
  if (document === history.present) return history;
  return {
    past: [...history.past.slice(1 - HISTORY_LIMIT), history.present],
    present: document,
    future: [],
  };
}

export function undo(history: History): History {
  const present = history.past.at(-1);
  return present
    ? {
        past: history.past.slice(0, -1),
        present,
        future: [history.present, ...history.future],
      }
    : history;
}

export function redo(history: History): History {
  const [present, ...future] = history.future;
  return present ? { past: [...history.past, history.present], present, future } : history;
}
