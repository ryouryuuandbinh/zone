import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type ChangeEvent,
  type KeyboardEvent,
  type PointerEvent,
} from 'react';
import { flushSync } from 'react-dom';
import {
  commit,
  createDocument,
  createHistory,
  defaultAnnotationStyle,
  hitTest,
  outputSize,
  redo,
  resizeAnnotation,
  undo,
  updateCrop,
  type Annotation,
  type AnnotationDocument,
  type AnnotationStyle,
  type Rect,
  type Tool,
} from './model';
import {
  cropGesture,
  gestureDocument,
  handlePoints,
  hitHandle,
  replaceAnnotation,
  viewportPoint,
  type Gesture,
} from './gestures';
import { EditorControls } from './EditorControls';
import { composeDocument, drawDocument } from './render';
import './editor.css';

export type EditorFormat = 'png' | 'jpeg';
export type EditorExportAction = 'copy' | 'save' | 'pin';

export interface EditorExport {
  action: EditorExportAction;
  blob: Blob;
  format: EditorFormat;
  width: number;
  height: number;
}

export interface EditorProps {
  frame: { id: string; width: number; height: number };
  sourceUrl: string;
  onExport: (exported: EditorExport) => Promise<boolean>;
  onCancel: () => Promise<void> | void;
  onReady?: () => Promise<void> | void;
  onError?: (message: string) => Promise<void> | void;
}

const MAX_IMAGE_BYTES = 8 * 1024 * 1024;
const MAX_ASSET_PIXELS = 12 * 1024 * 1024;
const MAX_ASSETS = 12;
const MAX_ANNOTATIONS = 240;
const MAX_STROKE_POINTS = 4096;
const clickTools: Tool[] = ['text', 'bubble', 'step', 'image', 'emoji', 'freehand'];

interface ImageAsset {
  id: string;
  name: string;
}

function messageFor(error: unknown): string {
  if (error instanceof Error && error.message) return error.message;
  return typeof error === 'string' ? error : 'Zone không thể hoàn tất thao tác ảnh.';
}

function isEditing(target: EventTarget | null): boolean {
  return (
    target instanceof HTMLElement &&
    Boolean(
      target.closest('input, select, textarea, [contenteditable]:not([contenteditable="false"])'),
    )
  );
}

function stagePoint(event: PointerEvent<HTMLDivElement>, frame: { width: number; height: number }) {
  return viewportPoint(
    { x: event.clientX, y: event.clientY },
    event.currentTarget.getBoundingClientRect(),
    frame,
  );
}

function Outline({
  box,
  scale,
  variant,
  handles,
}: {
  box: Rect;
  scale: number;
  variant: 'crop' | 'object';
  handles: boolean;
}) {
  const side = 7 / Math.max(scale, 0.02);
  return (
    <g className={`shot-outline shot-outline-${variant}`}>
      <rect className="shot-outline-border" {...box} vectorEffect="non-scaling-stroke" />
      {handles &&
        handlePoints(box).map(({ id, x, y }) => (
          <rect
            key={id}
            className="shot-outline-handle"
            x={x - side / 2}
            y={y - side / 2}
            width={side}
            height={side}
            rx={side / 5}
            vectorEffect="non-scaling-stroke"
          />
        ))}
    </g>
  );
}

function EditorSession({ frame, sourceUrl, onExport, onCancel, onReady, onError }: EditorProps) {
  const [history, setHistory] = useState(() =>
    createHistory(createDocument(frame.width, frame.height)),
  );
  const [draft, setDraft] = useState<AnnotationDocument | null>(null);
  const [tool, setTool] = useState<Tool>('crop');
  const [selectedId, setSelectedId] = useState<string>();
  const [style, setStyle] = useState<AnnotationStyle>({ ...defaultAnnotationStyle });
  const [newText, setNewText] = useState('');
  const [asset, setAsset] = useState<ImageAsset>();
  const [assetNames, setAssetNames] = useState<Record<string, string>>({});
  const [source, setSource] = useState<HTMLImageElement>();
  const [format, setFormat] = useState<EditorFormat>('png');
  const [viewport, setViewport] = useState(() => ({
    width: window.innerWidth,
    height: window.innerHeight,
  }));
  const [painted, setPainted] = useState(false);
  const [fatal, setFatal] = useState('');
  const [status, setStatus] = useState('');
  const [busy, setBusy] = useState(false);
  const editor = useRef<HTMLElement>(null);
  const canvas = useRef<HTMLCanvasElement>(null);
  const fileInput = useRef<HTMLInputElement>(null);
  const assets = useRef(new Map<string, ImageBitmap>());
  const gesture = useRef<Gesture | null>(null);
  const nextId = useRef(1);
  const presented = useRef(false);
  const inFlight = useRef(false);
  const mounted = useRef(false);
  const committed = useRef(history.present);

  const visible = draft ?? history.present;
  const crop = visible.crop;
  const hasSelection = Boolean(history.present.crop);
  const activeTool = hasSelection ? tool : 'crop';
  const selected = visible.annotations.find(({ id }) => id === selectedId);
  const scale = Math.max(
    0.02,
    Math.min(viewport.width / frame.width, viewport.height / frame.height),
  );
  const drawScale = Math.min(
    1,
    scale * window.devicePixelRatio,
    1600 / Math.max(frame.width, frame.height),
  );
  const ready = painted && !fatal;

  useLayoutEffect(() => {
    committed.current = history.present;
  }, [history.present]);

  useEffect(() => {
    mounted.current = true;
    const cache = assets.current;
    const resize = () => setViewport({ width: window.innerWidth, height: window.innerHeight });
    window.addEventListener('resize', resize);
    return () => {
      mounted.current = false;
      window.removeEventListener('resize', resize);
      for (const image of cache.values()) image.close();
      cache.clear();
    };
  }, []);

  useEffect(() => {
    let disposed = false;
    const image = new Image();
    image.crossOrigin = 'anonymous';
    const fail = () => {
      if (disposed) return;
      const message = 'Không mở được ảnh màn hình đã đóng băng.';
      setFatal(message);
      void Promise.resolve()
        .then(() => onError?.(message))
        .catch(() => {});
    };
    image.onload = () => {
      if (disposed) return;
      if (image.naturalWidth !== frame.width || image.naturalHeight !== frame.height) fail();
      else setSource(image);
    };
    image.onerror = fail;
    image.src = sourceUrl;
    return () => {
      disposed = true;
      image.onload = null;
      image.onerror = null;
    };
  }, [frame.height, frame.width, onError, sourceUrl]);

  useLayoutEffect(() => {
    if (!source || !canvas.current) return;
    let disposed = false;
    const surface = canvas.current;
    const paint = () => {
      try {
        drawDocument(surface, source, visible, assets.current, drawScale);
        if (!presented.current) {
          presented.current = true;
          queueMicrotask(() => {
            if (!mounted.current) return;
            setPainted(true);
            editor.current?.focus({ preventScroll: true });
            void Promise.resolve()
              .then(() => onReady?.())
              .catch((cause: unknown) => {
                if (!mounted.current) return;
                const message = messageFor(cause);
                setFatal(message);
                return onError?.(message);
              })
              .catch(() => {});
          });
        }
      } catch (cause) {
        const message = messageFor(cause);
        queueMicrotask(() => {
          if (disposed || !mounted.current) return;
          setFatal(message);
          void Promise.resolve()
            .then(() => onError?.(message))
            .catch(() => {});
        });
      }
    };
    // The hidden live window must paint once before showing. Once visible, coalesce pointer events.
    if (!presented.current) {
      paint();
      return () => {
        disposed = true;
      };
    }
    const request = window.requestAnimationFrame(paint);
    return () => {
      disposed = true;
      window.cancelAnimationFrame(request);
    };
  }, [drawScale, onError, onReady, source, visible]);

  function updateDocument(next: AnnotationDocument) {
    setHistory((current) => commit(current, next));
    setStatus('');
  }

  function applyItem(item: Annotation) {
    updateDocument(replaceAnnotation(history.present, item));
  }

  function pickTool(next: Tool) {
    setTool(next);
    if (next !== 'select') setSelectedId(undefined);
    setNewText('');
    setStatus('');
    if (next === 'image' && !asset) fileInput.current?.click();
  }

  function updateStyle<K extends keyof AnnotationStyle>(key: K, value: AnnotationStyle[K]) {
    if (selected) applyItem({ ...selected, style: { ...selected.style, [key]: value } });
    else setStyle((current) => ({ ...current, [key]: value }));
  }

  function deleteSelected() {
    if (!selectedId) return;
    const annotations = history.present.annotations.filter(({ id }) => id !== selectedId);
    if (annotations.length === history.present.annotations.length) return;
    updateDocument({ ...history.present, annotations });
    setSelectedId(undefined);
  }

  function clearGesture() {
    gesture.current = null;
    setDraft(null);
  }

  function undoEdit() {
    clearGesture();
    setHistory(undo);
    setStatus('');
  }

  function redoEdit() {
    clearGesture();
    setHistory(redo);
    setStatus('');
  }

  function focusEditor() {
    const focus = () => editor.current?.focus({ preventScroll: true });
    // A number field can commit a pending edit when focus leaves it.
    if (isEditing(document.activeElement)) flushSync(focus);
    else focus();
  }

  function start(event: PointerEvent<HTMLDivElement>) {
    if (!ready || busy || gesture.current || !event.isPrimary || event.button !== 0) return;
    const point = stagePoint(event, frame);
    focusEditor();
    const base = committed.current;
    const common = { base, start: point, pointerId: event.pointerId };
    const tolerance = 9 / scale;
    let next: Gesture;
    if (activeTool === 'crop') {
      next = cropGesture(base, point, event.pointerId, tolerance);
    } else if (activeTool === 'select') {
      const selectedItem = base.annotations.find(({ id }) => id === selectedId);
      const handle = selectedItem && hitHandle(selectedItem.box, point, tolerance);
      if (selectedItem && handle) {
        next = { ...common, kind: 'resize', item: selectedItem, handle };
      } else {
        const item = hitTest(base, point, tolerance);
        setSelectedId(item?.id);
        if (!item) return;
        next = { ...common, kind: 'move', item };
      }
    } else if (activeTool === 'eraser') {
      next = { ...common, kind: 'erase', erased: new Set(), tolerance };
    } else {
      if (activeTool === 'image' && !asset) {
        setStatus('Chọn ảnh trên máy trước khi đặt vào ảnh chụp.');
        return;
      }
      if (base.annotations.length >= MAX_ANNOTATIONS) {
        setStatus('Ảnh chụp đã đạt giới hạn chú thích.');
        return;
      }
      const sequence = base.annotations.reduce(
        (highest, item) =>
          item.tool === 'step' ? Math.max(highest, Number(item.text) || 0) : highest,
        0,
      );
      const text = newText.trim() || (activeTool === 'step' ? String(sequence + 1) : undefined);
      next = {
        ...common,
        kind: 'draw',
        id: `a-${nextId.current++}`,
        tool: activeTool,
        style,
        points: [point],
        text,
        assetId: asset?.id,
      };
      setSelectedId(undefined);
    }
    event.currentTarget.setPointerCapture(event.pointerId);
    gesture.current = next;
    if (next.kind === 'erase') setDraft(gestureDocument(next, point));
    event.preventDefault();
  }

  function move(event: PointerEvent<HTMLDivElement>) {
    const active = gesture.current;
    if (!active || active.pointerId !== event.pointerId) return;
    const point = stagePoint(event, frame);
    if (
      active.kind === 'draw' &&
      active.tool === 'freehand' &&
      active.points.length < MAX_STROKE_POINTS
    ) {
      const last = active.points.at(-1) ?? active.start;
      if (Math.hypot(point.x - last.x, point.y - last.y) >= 0.5 / scale) active.points.push(point);
    }
    setDraft(gestureDocument(active, point));
  }

  function finish(event: PointerEvent<HTMLDivElement>) {
    const active = gesture.current;
    if (!active || active.pointerId !== event.pointerId) return;
    const point = stagePoint(event, frame);
    const moved = Math.hypot(point.x - active.start.x, point.y - active.start.y) >= 2 / scale;
    clearGesture();
    if (
      active.kind === 'new-crop' &&
      (Math.abs(point.x - active.start.x) < 2 / scale ||
        Math.abs(point.y - active.start.y) < 2 / scale)
    ) {
      return;
    }
    if (
      !moved &&
      active.kind !== 'erase' &&
      (active.kind !== 'draw' || !clickTools.includes(active.tool))
    )
      return;
    if (
      active.kind === 'draw' &&
      active.tool === 'freehand' &&
      active.points.length < MAX_STROKE_POINTS
    ) {
      active.points.push(point);
    }
    updateDocument(gestureDocument(active, point));
    if (active.kind === 'draw') setSelectedId(active.id);
  }

  async function importImage(event: ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    event.target.value = '';
    if (!file || inFlight.current) return;
    if (file.size === 0 || file.size > MAX_IMAGE_BYTES || assets.current.size >= MAX_ASSETS) {
      setStatus('Chọn ảnh PNG, JPEG hoặc WebP tối đa 8 MiB. Mỗi phiên hỗ trợ tối đa 12 ảnh chèn.');
      return;
    }
    if (!['image/png', 'image/jpeg', 'image/webp'].includes(file.type)) {
      setStatus('Zone hỗ trợ ảnh chèn PNG, JPEG và WebP.');
      return;
    }
    inFlight.current = true;
    setBusy(true);
    try {
      const image = await createImageBitmap(file);
      if (!mounted.current) {
        image.close();
        return;
      }
      const used = [...assets.current.values()].reduce(
        (sum, item) => sum + item.width * item.height,
        0,
      );
      if (used + image.width * image.height > MAX_ASSET_PIXELS) {
        image.close();
        setStatus('Các ảnh chèn vượt giới hạn 12 megapixel của phiên chỉnh sửa.');
        return;
      }
      const id = `asset-${nextId.current++}`;
      assets.current.set(id, image);
      setAssetNames((names) => ({ ...names, [id]: file.name }));
      setAsset({ id, name: file.name });
      if (selected?.tool === 'image') applyItem({ ...selected, assetId: id });
      setTool('image');
      setStatus('Kéo trên ảnh chụp để đặt ảnh chèn.');
    } catch (cause) {
      if (mounted.current) setStatus(messageFor(cause));
    } finally {
      inFlight.current = false;
      if (mounted.current) setBusy(false);
    }
  }

  async function exportImage(action: EditorExportAction) {
    const doc = committed.current;
    if (!source || !ready || !doc.crop || gesture.current || inFlight.current) return;
    const area = outputSize(doc);
    if (!area.width || !area.height) return;
    inFlight.current = true;
    setBusy(true);
    setStatus('Đang kết xuất ảnh…');
    const outFormat = action === 'save' ? format : 'png';
    try {
      const blob = await composeDocument(
        source,
        doc,
        assets.current,
        outFormat === 'jpeg' ? 'image/jpeg' : 'image/png',
      );
      if (!mounted.current) return;
      const exported = await onExport({
        action,
        blob,
        format: outFormat,
        width: area.width,
        height: area.height,
      });
      if (!mounted.current) return;
      if (exported) {
        setStatus(
          action === 'copy'
            ? 'Đã sao chép ảnh.'
            : action === 'pin'
              ? 'Đã ghim ảnh.'
              : 'Đã lưu ảnh.',
        );
        await onCancel();
      } else {
        setStatus('Đã hủy thao tác.');
      }
    } catch (cause) {
      if (mounted.current) setStatus(messageFor(cause));
    } finally {
      inFlight.current = false;
      if (mounted.current) setBusy(false);
    }
  }

  async function cancel() {
    if (inFlight.current) return;
    inFlight.current = true;
    setBusy(true);
    try {
      await onCancel();
    } catch (cause) {
      if (mounted.current) setStatus(messageFor(cause));
    } finally {
      inFlight.current = false;
      if (mounted.current) setBusy(false);
    }
  }

  function shortcut(event: KeyboardEvent<HTMLElement>) {
    if (event.nativeEvent.isComposing || event.defaultPrevented) return;
    if (event.key === 'Escape') {
      event.preventDefault();
      if (event.repeat) return;
      if (gesture.current) clearGesture();
      else void cancel();
      return;
    }
    const target = event.target;
    const editing = isEditing(target);
    const key = event.key.toLowerCase();
    const control = event.ctrlKey && !event.altKey && !event.metaKey;
    if (control && !event.shiftKey && key === 's') {
      event.preventDefault();
      if (!hasSelection || busy || !ready || gesture.current || event.repeat) return;
      // Number fields apply their edits on blur. Flush that edit before reading the crop.
      if (editing) focusEditor();
      void exportImage('save');
      return;
    }
    if (editing) return;
    if (busy || !ready) return;
    const plainEnter =
      key === 'enter' &&
      !event.ctrlKey &&
      !event.altKey &&
      !event.metaKey &&
      !event.shiftKey &&
      !(target instanceof Element && target.closest('button, a[href], [role="button"]'));
    if (hasSelection && ((control && !event.shiftKey && key === 'c') || plainEnter)) {
      event.preventDefault();
      if (!event.repeat && !gesture.current) void exportImage('copy');
    } else if (control && (key === 'z' || key === 'y')) {
      event.preventDefault();
      if (key === 'y' || event.shiftKey) redoEdit();
      else undoEdit();
    } else if (event.key === 'Delete') {
      event.preventDefault();
      deleteSelected();
    }
  }

  const full = `M 0 0 H ${frame.width} V ${frame.height} H 0 Z`;
  const interior = crop
    ? `M ${crop.x} ${crop.y} h ${crop.width} v ${crop.height} h ${-crop.width} Z`
    : '';
  const output = outputSize(visible);
  const cutoutStatus =
    crop && (output.width !== crop.width || output.height !== crop.height)
      ? output.width > 0 && output.height > 0
        ? `Kích thước xuất sau khi cắt bỏ: ${output.width} × ${output.height} px.`
        : 'Dải cắt bỏ đang chiếm toàn bộ vùng chụp.'
      : '';
  const activeAssetName = selected?.assetId ? assetNames[selected.assetId] : asset?.name;

  return (
    <main
      ref={editor}
      className="shot-editor"
      aria-label="Trình chỉnh sửa ảnh chụp Zone"
      onKeyDown={shortcut}
      tabIndex={-1}
    >
      <div className="shot-stage-holder">
        <div
          className={`shot-stage shot-tool-${activeTool}`}
          style={{ width: frame.width * scale, height: frame.height * scale }}
          onPointerDown={start}
          onPointerMove={move}
          onPointerUp={finish}
          onPointerCancel={clearGesture}
          onLostPointerCapture={clearGesture}
        >
          <canvas ref={canvas} aria-label="Ảnh màn hình đã đóng băng" />
          <svg
            className="shot-stage-guides"
            viewBox={`0 0 ${frame.width} ${frame.height}`}
            aria-hidden="true"
          >
            {crop ? (
              <>
                <path className="shot-crop-mask" fillRule="evenodd" d={`${full} ${interior}`} />
                <Outline box={crop} scale={scale} variant="crop" handles={activeTool === 'crop'} />
              </>
            ) : (
              <path className="shot-crop-mask" d={full} />
            )}
            {selected && activeTool !== 'crop' && (
              <Outline
                box={selected.box}
                scale={scale}
                variant="object"
                handles={activeTool === 'select'}
              />
            )}
          </svg>
          {crop && (
            <span
              className={`shot-region-size${crop.y * scale < 28 ? ' inside' : ''}`}
              style={{ left: crop.x * scale, top: crop.y * scale }}
              aria-label={`Vùng chụp ${crop.width} nhân ${crop.height} pixel`}
            >
              {crop.width} × {crop.height} px
            </span>
          )}
        </div>
      </div>

      {!ready && (
        <p className="shot-loading" role={fatal ? 'alert' : 'status'}>
          {fatal || 'Đang mở ảnh đã đóng băng…'}
        </p>
      )}
      {ready && !hasSelection && (
        <p className="shot-selection-hint" role="status">
          {status || 'Kéo để chọn vùng · Esc để hủy'}
        </p>
      )}
      {hasSelection && crop && (
        <div className="shot-bottom">
          <p className="shot-status" role="status">
            {status || cutoutStatus}
          </p>
          <EditorControls
            tool={activeTool}
            selected={selected}
            style={selected?.style ?? style}
            text={selected ? (selected.text ?? '') : newText}
            assetName={activeAssetName}
            crop={crop}
            frame={frame}
            format={format}
            canUndo={history.past.length > 0}
            canRedo={history.future.length > 0}
            canExport={output.width > 0 && output.height > 0}
            disabled={!ready || busy || Boolean(draft)}
            onTool={pickTool}
            onStyle={updateStyle}
            onText={(text) => (selected ? applyItem({ ...selected, text }) : setNewText(text))}
            onBox={(box) => {
              if (selected) applyItem(resizeAnnotation(selected, box));
            }}
            onCrop={(box) => updateDocument(updateCrop(history.present, box))}
            onImport={() => fileInput.current?.click()}
            onUndo={undoEdit}
            onRedo={redoEdit}
            onDelete={deleteSelected}
            onFormat={setFormat}
            onCopy={() => void exportImage('copy')}
            onSave={() => void exportImage('save')}
            onPin={() => void exportImage('pin')}
            onCancel={() => void cancel()}
          />
        </div>
      )}
      <input
        ref={fileInput}
        type="file"
        accept="image/png,image/jpeg,image/webp"
        hidden
        onChange={(event) => void importImage(event)}
      />
    </main>
  );
}

export function Editor(props: EditorProps) {
  return <EditorSession key={props.frame.id} {...props} />;
}
