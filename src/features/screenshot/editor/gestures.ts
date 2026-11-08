import {
  boxFromPoints,
  clampCrop,
  containsPoint,
  createAnnotation,
  hitTest,
  moveAnnotation,
  moveCrop,
  resizeAnnotation,
  updateCrop,
  type Annotation,
  type AnnotationDocument,
  type AnnotationStyle,
  type AnnotationTool,
  type Point,
  type Rect,
} from './model';

const handles = ['nw', 'n', 'ne', 'e', 'se', 's', 'sw', 'w'] as const;
export type Handle = (typeof handles)[number];

export function viewportPoint(
  client: Point,
  surface: { left: number; top: number; width: number; height: number },
  frame: { width: number; height: number },
): Point {
  if (surface.width <= 0 || surface.height <= 0) return { x: 0, y: 0 };
  const x = ((client.x - surface.left) / surface.width) * frame.width;
  const y = ((client.y - surface.top) / surface.height) * frame.height;
  return { x: Math.max(0, Math.min(frame.width, x)), y: Math.max(0, Math.min(frame.height, y)) };
}

export function handlePoints(box: Rect) {
  return handles.map((id) => ({
    id,
    x: box.x + (id.includes('w') ? 0 : id.includes('e') ? box.width : box.width / 2),
    y: box.y + (id.includes('n') ? 0 : id.includes('s') ? box.height : box.height / 2),
  }));
}

export function hitHandle(box: Rect, point: Point, tolerance: number): Handle | undefined {
  return handlePoints(box).find(
    (handle) => Math.hypot(handle.x - point.x, handle.y - point.y) <= tolerance,
  )?.id;
}

function resizeBox(box: Rect, handle: Handle, dx: number, dy: number): Rect {
  const right = box.x + box.width;
  const bottom = box.y + box.height;
  const x = handle.includes('w') ? Math.min(right - 1, box.x + dx) : box.x;
  const y = handle.includes('n') ? Math.min(bottom - 1, box.y + dy) : box.y;
  return {
    x,
    y,
    width: (handle.includes('e') ? Math.max(x + 1, right + dx) : right) - x,
    height: (handle.includes('s') ? Math.max(y + 1, bottom + dy) : bottom) - y,
  };
}

type Draw = {
  kind: 'draw';
  id: string;
  tool: AnnotationTool;
  style: AnnotationStyle;
  points: Point[];
  text?: string;
  assetId?: string;
};

export type Gesture = {
  pointerId: number;
  start: Point;
  base: AnnotationDocument;
} & (
  | Draw
  | { kind: 'new-crop' }
  | { kind: 'move-crop' }
  | { kind: 'resize-crop'; handle: Handle }
  | { kind: 'move'; item: Annotation }
  | { kind: 'resize'; item: Annotation; handle: Handle }
  | { kind: 'erase'; erased: Set<string>; tolerance: number; lastSample?: Point }
);

export function replaceAnnotation(doc: AnnotationDocument, item: Annotation): AnnotationDocument {
  return { ...doc, annotations: doc.annotations.map((old) => (old.id === item.id ? item : old)) };
}

export function cropGesture(
  base: AnnotationDocument,
  point: Point,
  pointerId: number,
  tolerance: number,
): Gesture {
  const common = { start: point, pointerId, base };
  if (base.crop) {
    const handle = hitHandle(base.crop, point, tolerance);
    if (handle) return { ...common, kind: 'resize-crop', handle };
    if (containsPoint(base.crop, point)) return { ...common, kind: 'move-crop' };
  }
  return { ...common, kind: 'new-crop' };
}

function placedEnd(gesture: Gesture & Draw, point: Point): Point {
  if (Math.hypot(point.x - gesture.start.x, point.y - gesture.start.y) >= 2) return point;
  const { fontSize } = gesture.style;
  switch (gesture.tool) {
    case 'text':
    case 'bubble':
      return { x: gesture.start.x + fontSize * 8, y: gesture.start.y + fontSize * 2 };
    case 'step':
    case 'emoji':
      return { x: gesture.start.x + fontSize * 2, y: gesture.start.y + fontSize * 2 };
    case 'image':
      return { x: gesture.start.x + 180, y: gesture.start.y + 120 };
    default:
      return point;
  }
}

export function gestureDocument(gesture: Gesture, point: Point): AnnotationDocument {
  const { base, start } = gesture;
  const dx = point.x - start.x;
  const dy = point.y - start.y;
  switch (gesture.kind) {
    case 'new-crop':
      return updateCrop(base, clampCrop(boxFromPoints(start, point), base.width, base.height));
    case 'move-crop':
      return moveCrop(base, dx, dy);
    case 'resize-crop':
      return base.crop
        ? updateCrop(
            base,
            clampCrop(resizeBox(base.crop, gesture.handle, dx, dy), base.width, base.height),
          )
        : base;
    case 'move':
      return replaceAnnotation(base, moveAnnotation(gesture.item, dx, dy));
    case 'resize':
      return replaceAnnotation(
        base,
        resizeAnnotation(gesture.item, resizeBox(gesture.item.box, gesture.handle, dx, dy)),
      );
    case 'erase': {
      const doc = gesture.erased.size
        ? { ...base, annotations: base.annotations.filter(({ id }) => !gesture.erased.has(id)) }
        : base;
      const last = gesture.lastSample;
      if (last && Math.hypot(last.x - point.x, last.y - point.y) < 0.5) return doc;
      gesture.lastSample = point;
      const item = hitTest(doc, point, gesture.tolerance);
      if (!item) return doc;
      gesture.erased.add(item.id);
      return { ...doc, annotations: doc.annotations.filter(({ id }) => id !== item.id) };
    }
    case 'draw': {
      const item = createAnnotation(
        gesture.id,
        gesture.tool,
        start,
        placedEnd(gesture, point),
        gesture.style,
        {
          points: gesture.tool === 'freehand' ? gesture.points : undefined,
          text: gesture.text,
          assetId: gesture.assetId,
        },
      );
      return { ...base, annotations: [...base.annotations, item] };
    }
  }
}
