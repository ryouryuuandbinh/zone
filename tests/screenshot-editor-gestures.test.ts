import { describe, expect, it } from 'vitest';
import {
  cropGesture,
  gestureDocument,
  viewportPoint,
  type Gesture,
} from '../src/features/screenshot/editor/gestures';
import {
  createAnnotation,
  createDocument,
  updateCrop,
} from '../src/features/screenshot/editor/model';

describe('screenshot editor viewport gestures', () => {
  it('maps the same point after a DPI or editor viewport change and clamps pointer capture to source edges', () => {
    const frame = { width: 1920, height: 1080 };
    const live = { left: 0, top: 0, width: 1280, height: 720 };
    const post = { left: 120, top: 80, width: 960, height: 540 };
    const source = { x: 960, y: 270 };
    expect(viewportPoint({ x: 640, y: 180 }, live, frame)).toEqual(source);
    expect(viewportPoint({ x: 600, y: 215 }, post, frame)).toEqual(source);
    expect(viewportPoint({ x: 34, y: 702 }, post, frame)).toEqual({ x: 0, y: 1080 });
  });

  it('moves and resizes a selected crop without translating existing annotations', () => {
    const base = createDocument(1920, 1080);
    const rectangle = createAnnotation('box', 'rect', { x: 180, y: 140 }, { x: 280, y: 240 });
    const initial = updateCrop(
      { ...base, annotations: [rectangle] },
      { x: 100, y: 100, width: 300, height: 260 },
    );
    const move = cropGesture(initial, { x: 200, y: 200 }, 7, 10);
    const moved = gestureDocument(move, { x: 230, y: 215 });
    expect(moved.crop).toEqual({ x: 130, y: 115, width: 300, height: 260 });
    const resize = cropGesture(moved, { x: 430, y: 375 }, 7, 10);
    const resized = gestureDocument(resize, { x: 510, y: 410 });
    expect(resized.crop).toEqual({ x: 130, y: 115, width: 380, height: 295 });
    expect(resized.annotations).toEqual([rectangle]);
  });

  it('erases only the top layer when pointer down and up sample the same point', () => {
    const start = { x: 80, y: 80 };
    const end = { x: 200, y: 200 };
    const bottom = createAnnotation('bottom', 'rect', start, end);
    const top = createAnnotation('top', 'ellipse', start, end);
    const point = { x: 130, y: 130 };
    const gesture: Gesture = {
      pointerId: 1,
      kind: 'erase',
      start: point,
      base: { ...createDocument(480, 320), annotations: [bottom, top] },
      tolerance: 8,
      erased: new Set(),
    };
    expect(gestureDocument(gesture, point).annotations).toEqual([bottom]);
    expect(gestureDocument(gesture, point).annotations).toEqual([bottom]);
  });
});
