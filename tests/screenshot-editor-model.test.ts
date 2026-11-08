import { describe, expect, it } from 'vitest';
import {
  clampCrop,
  commit,
  createAnnotation,
  createDocument,
  createHistory,
  defaultAnnotationStyle,
  hitTest,
  moveAnnotation,
  moveCrop,
  outputLayout,
  outputSize,
  redo,
  resizeAnnotation,
  undo,
  updateCrop,
} from '../src/features/screenshot/editor/model';

describe('screenshot editor document', () => {
  it('moves and resizes the physical crop without moving annotations', () => {
    const mark = createAnnotation('a', 'rect', { x: 30, y: 30 }, { x: 50, y: 65 });
    const annotated = { ...createDocument(1440, 900), annotations: [mark] };
    const cropped = updateCrop(annotated, { x: 8.8, y: 16.2, width: 380, height: 250 });
    expect(cropped.crop).toEqual({ x: 8, y: 16, width: 381, height: 251 });

    const moved = moveCrop(cropped, 2000, -1000);
    expect(moved.crop).toEqual({ x: 1059, y: 0, width: 381, height: 251 });
    const resized = updateCrop(moved, { x: 1059, y: 0, width: 150, height: 250 });
    expect(resized.crop).toEqual({ x: 1059, y: 0, width: 150, height: 250 });
    expect(resized.annotations[0]).toBe(mark);
    expect(clampCrop({ x: 1439.2, y: 899.7, width: 3, height: 3 }, 1440, 900)).toEqual({
      x: 1439,
      y: 899,
      width: 1,
      height: 1,
    });
  });

  it('transforms stroke points when moving or resizing and hits the visible line', () => {
    const mark = createAnnotation('a', 'arrow', { x: 20, y: 20 }, { x: 60, y: 60 });
    const translated = moveAnnotation(mark, 12, 10);
    const resized = resizeAnnotation(translated, { x: 32, y: 30, width: 80, height: 20 });
    expect(mark.points).toEqual([
      { x: 20, y: 20 },
      { x: 60, y: 60 },
    ]);
    expect(resized.points).toEqual([
      { x: 32, y: 30 },
      { x: 112, y: 50 },
    ]);
    const document = { ...createDocument(300, 200), annotations: [resized] };
    expect(hitTest(document, { x: 72, y: 39 })?.id).toBe('a');
    expect(hitTest(document, { x: 103, y: 31 })).toBeNull();
  });

  it('undoes geometry and layer edits together and clears the redo branch after a new edit', () => {
    const start = createDocument(300, 200);
    const mark = createAnnotation('one', 'freehand', { x: 20, y: 20 }, { x: 60, y: 60 });
    const withMark = { ...start, annotations: [mark] };
    const selected = updateCrop(withMark, { x: 5, y: 4, width: 50, height: 25 });
    const history = commit(commit(createHistory(start), withMark), selected);
    expect(undo(history).present).toBe(withMark);
    expect(redo(undo(history)).present).toBe(selected);
    expect(undo(undo(history)).present).toBe(start);

    const replacement = updateCrop(withMark, { x: 6, y: 7, width: 60, height: 41 });
    const branched = commit(undo(history), replacement);
    expect(redo(branched).present).toBe(replacement);
    expect(branched.future).toEqual([]);
  });

  it('merges overlapping cut-out strips and stitches only retained source spans', () => {
    const band = (
      id: string,
      direction: 'horizontal' | 'vertical',
      from: { x: number; y: number },
      to: { x: number; y: number },
    ) =>
      createAnnotation(id, 'cutout', from, to, {
        ...defaultAnnotationStyle,
        cutoutDirection: direction,
      });
    const crop = updateCrop(createDocument(100, 100), { x: 10, y: 10, width: 80, height: 70 });
    const doc = {
      ...crop,
      annotations: [
        band('a', 'horizontal', { x: 20, y: 20 }, { x: 24, y: 30 }),
        band('b', 'horizontal', { x: 25, y: 25 }, { x: 30, y: 38 }),
        band('c', 'vertical', { x: 50, y: 24 }, { x: 65, y: 30 }),
        band('d', 'vertical', { x: 93, y: 24 }, { x: 98, y: 30 }),
      ],
    };
    expect(outputSize(doc)).toEqual({ width: 65, height: 52 });
    expect(outputLayout(doc).columns).toEqual([
      { start: 10, length: 40, offset: 0 },
      { start: 65, length: 25, offset: 40 },
    ]);
    expect(outputLayout(doc).rows).toEqual([
      { start: 10, length: 10, offset: 0 },
      { start: 38, length: 42, offset: 10 },
    ]);
    expect(doc.crop).toBe(crop.crop);
    expect(hitTest(doc, { x: 80, y: 25 })?.id).toBe('b');
  });
});
