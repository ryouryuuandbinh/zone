import {
  cutoutGuide,
  outputLayout,
  type Annotation,
  type AnnotationDocument,
  type Rect,
} from './model';

export type ImageFormat = 'image/png' | 'image/jpeg';
export type ImageAssets = ReadonlyMap<string, CanvasImageSource>;

const MAX_PIXELS = 16 * 1024 * 1024;
const MAX_SIDE = 16_384;
const MAX_EXPORT_BYTES = 32 * 1024 * 1024;

interface RenderContext {
  canvas: HTMLCanvasElement;
  ctx: CanvasRenderingContext2D;
  document: AnnotationDocument;
  scaleX: number;
  scaleY: number;
  pixelBuffer?: HTMLCanvasElement;
}

function canvasContext(canvas: HTMLCanvasElement): CanvasRenderingContext2D {
  const context = canvas.getContext('2d', { alpha: true });
  if (!context) throw new Error('Không thể mở vùng chỉnh sửa ảnh.');
  return context;
}

function checkCanvasDimensions(width: number, height: number) {
  if (
    !Number.isSafeInteger(width) ||
    !Number.isSafeInteger(height) ||
    width < 1 ||
    height < 1 ||
    width > MAX_SIDE ||
    height > MAX_SIDE ||
    width * height > MAX_PIXELS
  ) {
    throw new Error('Kích thước ảnh vượt giới hạn chỉnh sửa.');
  }
}

function ellipse(ctx: CanvasRenderingContext2D, box: Rect) {
  ctx.ellipse(
    box.x + box.width / 2,
    box.y + box.height / 2,
    box.width / 2,
    box.height / 2,
    0,
    0,
    Math.PI * 2,
  );
}

function contrastText(color: string) {
  if (!/^#[0-9a-f]{6}$/i.test(color)) return '#151515';
  const rgb = Number.parseInt(color.slice(1), 16);
  const luminance = ((rgb >> 16) * 0.299 + ((rgb >> 8) & 255) * 0.587 + (rgb & 255) * 0.114) / 255;
  return luminance > 0.58 ? '#151515' : '#fff';
}

function font(annotation: Annotation, emoji = false) {
  const { fontFamily, fontSize } = annotation.style;
  const family = fontFamily.replace(/["\\\r\n]/g, '');
  const fontStack = emoji ? '"Segoe UI Emoji", sans-serif' : `"${family}", sans-serif`;
  return `${Math.max(1, fontSize)}px ${fontStack}`;
}

function fillText(
  ctx: CanvasRenderingContext2D,
  annotation: Annotation,
  box: Rect,
  color: string,
  centered = false,
) {
  if (!annotation.text || box.width <= 0 || box.height <= 0) return;
  ctx.save();
  ctx.beginPath();
  ctx.rect(box.x, box.y, box.width, box.height);
  ctx.clip();
  ctx.fillStyle = color;
  ctx.font = font(annotation, annotation.tool === 'emoji');
  if (centered) {
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.fillText(annotation.text, box.x + box.width / 2, box.y + box.height / 2, box.width);
  } else {
    ctx.textAlign = 'left';
    ctx.textBaseline = 'top';
    const lineHeight = Math.max(1, annotation.style.fontSize) * 1.25;
    let y = box.y;
    for (const paragraph of annotation.text.split('\n')) {
      let line = '';
      for (const word of paragraph.split(/\s+/)) {
        const next = line ? `${line} ${word}` : word;
        if (line && ctx.measureText(next).width > box.width) {
          ctx.fillText(line, box.x, y, box.width);
          line = word;
          y += lineHeight;
        } else {
          line = next;
        }
        if (y >= box.y + box.height) break;
      }
      if (y >= box.y + box.height) break;
      ctx.fillText(line, box.x, y, box.width);
      y += lineHeight;
    }
  }
  ctx.restore();
}

function drawBubble(ctx: CanvasRenderingContext2D, annotation: Annotation) {
  const { box, style } = annotation;
  const tail = Math.min(box.height * 0.27, box.width * 0.22);
  const bottom = box.y + box.height - tail;
  const tailStart = box.x + box.width * 0.23;
  const tailWidth = Math.min(tail, box.width * 0.35);
  ctx.beginPath();
  ctx.moveTo(box.x, box.y);
  ctx.lineTo(box.x + box.width, box.y);
  ctx.lineTo(box.x + box.width, bottom);
  ctx.lineTo(tailStart + tailWidth, bottom);
  ctx.lineTo(tailStart, box.y + box.height);
  ctx.lineTo(tailStart, bottom);
  ctx.lineTo(box.x, bottom);
  ctx.closePath();
  ctx.fillStyle = style.fill ? style.color : 'rgba(0, 0, 0, 0.78)';
  ctx.fill();
  ctx.stroke();
  const padding = Math.min(Math.max(style.strokeWidth + 3, 5), box.width / 4, box.height / 4);
  fillText(
    ctx,
    annotation,
    {
      x: box.x + padding,
      y: box.y + padding,
      width: box.width - padding * 2,
      height: box.height - tail - padding * 2,
    },
    style.fill ? contrastText(style.color) : style.color,
  );
}

function drawPolyline(ctx: CanvasRenderingContext2D, annotation: Annotation) {
  const points = annotation.points;
  const first = points?.[0];
  if (!first || !points) return;

  ctx.beginPath();
  ctx.moveTo(first.x, first.y);
  let last = first;
  let previous = first;
  for (let index = 1; index < points.length; index++) {
    const point = points[index];
    if (!point || (point.x === last.x && point.y === last.y)) continue;
    ctx.lineTo(point.x, point.y);
    previous = last;
    last = point;
  }
  if (last === first) {
    ctx.beginPath();
    ctx.arc(first.x, first.y, ctx.lineWidth / 2, 0, Math.PI * 2);
    ctx.fill();
    return;
  }
  ctx.stroke();
  if (annotation.tool !== 'arrow') return;

  const angle = Math.atan2(last.y - previous.y, last.x - previous.x);
  const length = Math.min(
    Math.hypot(last.x - first.x, last.y - first.y) * 0.55,
    Math.max(10, ctx.lineWidth * 4),
  );
  ctx.beginPath();
  ctx.moveTo(last.x, last.y);
  ctx.lineTo(
    last.x - Math.cos(angle - Math.PI / 6) * length,
    last.y - Math.sin(angle - Math.PI / 6) * length,
  );
  ctx.lineTo(
    last.x - Math.cos(angle + Math.PI / 6) * length,
    last.y - Math.sin(angle + Math.PI / 6) * length,
  );
  ctx.closePath();
  ctx.fill();
}

function intersect(box: Rect, width: number, height: number): Rect | null {
  const x = Math.max(0, box.x);
  const y = Math.max(0, box.y);
  const right = Math.min(width, box.x + box.width);
  const bottom = Math.min(height, box.y + box.height);
  return right > x && bottom > y ? { x, y, width: right - x, height: bottom - y } : null;
}

function clipRect(ctx: CanvasRenderingContext2D, box: Rect) {
  ctx.beginPath();
  ctx.rect(box.x, box.y, box.width, box.height);
  ctx.clip();
}

function sample(
  target: RenderContext,
  destination: CanvasRenderingContext2D,
  source: Rect,
  box: Rect,
) {
  const { canvas, scaleX, scaleY } = target;
  destination.drawImage(
    canvas,
    source.x * scaleX,
    source.y * scaleY,
    source.width * scaleX,
    source.height * scaleY,
    box.x,
    box.y,
    box.width,
    box.height,
  );
}

function drawImageEffect(target: RenderContext, annotation: Annotation) {
  const { ctx, document, scaleX, scaleY } = target;
  const { box, style, tool } = annotation;
  const { width, height } = document;
  const visible = intersect(box, width, height);
  if (!visible) return;
  if (tool === 'magnifier') {
    const zoom = Math.min(8, 1 + Math.max(2, style.amount) / 12);
    const source: Rect = {
      x: box.x + (box.width - box.width / zoom) / 2,
      y: box.y + (box.height - box.height / zoom) / 2,
      width: box.width / zoom,
      height: box.height / zoom,
    };
    ctx.save();
    ctx.beginPath();
    ellipse(ctx, box);
    ctx.clip();
    const sampled = intersect(source, width, height);
    if (sampled) {
      sample(target, ctx, sampled, {
        x: box.x + (sampled.x - source.x) * zoom,
        y: box.y + (sampled.y - source.y) * zoom,
        width: sampled.width * zoom,
        height: sampled.height * zoom,
      });
    }
    ctx.restore();
    ctx.beginPath();
    ellipse(ctx, box);
    ctx.stroke();
    return;
  }

  clipRect(ctx, box);
  if (tool === 'blur') {
    const radius = Math.max(1, Math.min(128, style.amount));
    const padding = radius * 3;
    const source = intersect(
      {
        x: visible.x - padding,
        y: visible.y - padding,
        width: visible.width + padding * 2,
        height: visible.height + padding * 2,
      },
      width,
      height,
    );
    if (!source) return;
    ctx.filter = `blur(${radius * Math.max(scaleX, scaleY)}px)`;
    sample(target, ctx, source, source);
  } else {
    const blockSize = Math.max(2, Math.min(128, style.amount));
    const buffer = (target.pixelBuffer ??= window.document.createElement('canvas'));
    const smallWidth = Math.min(1024, Math.max(1, Math.ceil(visible.width / blockSize)));
    const smallHeight = Math.min(1024, Math.max(1, Math.ceil(visible.height / blockSize)));
    buffer.width = smallWidth;
    buffer.height = smallHeight;
    const small = canvasContext(buffer);
    small.imageSmoothingEnabled = true;
    sample(target, small, visible, { x: 0, y: 0, width: smallWidth, height: smallHeight });
    ctx.imageSmoothingEnabled = false;
    ctx.drawImage(buffer, visible.x, visible.y, visible.width, visible.height);
  }
}

function drawAnnotation(target: RenderContext, annotation: Annotation, assets?: ImageAssets) {
  const { ctx, document } = target;
  const { box, style, tool } = annotation;
  ctx.save();
  try {
    ctx.fillStyle = style.color;
    ctx.strokeStyle = style.color;
    ctx.globalAlpha = Math.max(0, Math.min(1, style.opacity));
    ctx.lineWidth = Math.max(1, style.strokeWidth);
    ctx.lineJoin = 'round';
    ctx.lineCap = 'round';
    switch (tool) {
      case 'rect':
        if (style.fill) ctx.fillRect(box.x, box.y, box.width, box.height);
        else ctx.strokeRect(box.x, box.y, box.width, box.height);
        break;
      case 'ellipse':
        ctx.beginPath();
        ellipse(ctx, box);
        if (style.fill) ctx.fill();
        else ctx.stroke();
        break;
      case 'line':
      case 'arrow':
      case 'freehand':
        drawPolyline(ctx, annotation);
        break;
      case 'text':
      case 'emoji':
        fillText(ctx, annotation, box, style.color, tool === 'emoji');
        break;
      case 'bubble':
        drawBubble(ctx, annotation);
        break;
      case 'step':
        ctx.beginPath();
        ellipse(ctx, box);
        ctx.fill();
        fillText(ctx, annotation, box, contrastText(style.color), true);
        break;
      case 'image': {
        const image = annotation.assetId ? assets?.get(annotation.assetId) : undefined;
        if (image) ctx.drawImage(image, box.x, box.y, box.width, box.height);
        break;
      }
      case 'highlight':
        ctx.globalAlpha *= 0.38;
        ctx.fillRect(box.x, box.y, box.width, box.height);
        break;
      case 'blur':
      case 'pixelate':
      case 'magnifier':
        drawImageEffect(target, annotation);
        break;
      case 'spotlight':
        ctx.globalAlpha *= 0.68;
        ctx.fillStyle = '#000';
        ctx.beginPath();
        ctx.rect(0, 0, document.width, document.height);
        ctx.moveTo(box.x + box.width, box.y + box.height / 2);
        ellipse(ctx, box);
        ctx.fill('evenodd');
        break;
      case 'cutout':
        // Geometry only. The export step omits the selected rows/columns.
        break;
    }
  } finally {
    ctx.restore();
  }
}

function renderDocument(
  canvas: HTMLCanvasElement,
  base: CanvasImageSource,
  document: AnnotationDocument,
  assets: ImageAssets | undefined,
  scale: number,
  showGuides: boolean,
): void {
  checkCanvasDimensions(document.width, document.height);
  if (!Number.isFinite(scale) || scale <= 0 || scale > 1) {
    throw new Error('Tỉ lệ xem trước không hợp lệ.');
  }
  const width = Math.max(1, Math.round(document.width * scale));
  const height = Math.max(1, Math.round(document.height * scale));
  if (canvas.width !== width) canvas.width = width;
  if (canvas.height !== height) canvas.height = height;
  const ctx = canvasContext(canvas);
  const scaleX = width / document.width;
  const scaleY = height / document.height;
  const target: RenderContext = { canvas, ctx, document, scaleX, scaleY };
  ctx.resetTransform();
  ctx.clearRect(0, 0, width, height);
  ctx.save();
  try {
    ctx.scale(scaleX, scaleY);
    ctx.drawImage(base, 0, 0, document.width, document.height);
    for (const annotation of document.annotations) drawAnnotation(target, annotation, assets);
    if (showGuides) {
      ctx.fillStyle = 'rgba(230, 58, 78, 0.30)';
      ctx.strokeStyle = 'rgba(255, 127, 146, 0.9)';
      ctx.lineWidth = 1 / Math.min(scaleX, scaleY);
      ctx.setLineDash([5 / scaleX, 4 / scaleX]);
      for (const annotation of document.annotations) {
        if (annotation.tool !== 'cutout') continue;
        const guide = cutoutGuide(document, annotation);
        if (!guide) continue;
        ctx.fillRect(guide.x, guide.y, guide.width, guide.height);
        ctx.strokeRect(guide.x, guide.y, guide.width, guide.height);
      }
    }
  } finally {
    ctx.restore();
    if (target.pixelBuffer) {
      target.pixelBuffer.width = 0;
      target.pixelBuffer.height = 0;
    }
  }
}

export function drawDocument(
  canvas: HTMLCanvasElement,
  base: CanvasImageSource,
  document: AnnotationDocument,
  assets?: ImageAssets,
  scale = 1,
): void {
  renderDocument(canvas, base, document, assets, scale, true);
}

function encode(canvas: HTMLCanvasElement, format: ImageFormat, quality: number): Promise<Blob> {
  return new Promise((resolve, reject) => {
    canvas.toBlob(
      (blob) => {
        if (!blob || blob.type !== format) {
          reject(new Error('Không thể mã hóa ảnh ở định dạng đã chọn.'));
        } else if (blob.size > MAX_EXPORT_BYTES) {
          reject(new Error('Ảnh xuất vượt quá 32 MiB. Hãy chọn vùng nhỏ hơn hoặc dùng JPEG.'));
        } else {
          resolve(blob);
        }
      },
      format,
      quality,
    );
  });
}

export async function composeDocument(
  base: CanvasImageSource,
  document: AnnotationDocument,
  assets?: ImageAssets,
  format: ImageFormat = 'image/png',
  quality = 0.92,
): Promise<Blob> {
  const rendered = window.document.createElement('canvas');
  let output = rendered;
  try {
    const layout = outputLayout(document);
    if (layout.width === 0 || layout.height === 0) {
      throw new Error('Các dải cắt đã loại bỏ toàn bộ ảnh. Hãy điều chỉnh vùng chọn.');
    }
    renderDocument(rendered, base, document, assets, 1, false);
    if (
      layout.width !== document.width ||
      layout.height !== document.height ||
      format === 'image/jpeg'
    ) {
      output = stitchImage(rendered, layout, format);
      rendered.width = 0;
      rendered.height = 0;
    }
    return await encode(output, format, quality);
  } finally {
    rendered.width = 0;
    rendered.height = 0;
    if (output !== rendered) {
      output.width = 0;
      output.height = 0;
    }
  }
}

function stitchImage(
  rendered: HTMLCanvasElement,
  layout: ReturnType<typeof outputLayout>,
  format: ImageFormat,
): HTMLCanvasElement {
  const { crop, columns, rows, width, height } = layout;
  const output = window.document.createElement('canvas');
  let intermediate: HTMLCanvasElement | undefined;
  try {
    // Splitting both axes can otherwise require columns.length * rows.length blits.
    if (columns.length > 1 && rows.length > 1) {
      intermediate = window.document.createElement('canvas');
      intermediate.width = width;
      intermediate.height = crop.height;
      const first = canvasContext(intermediate);
      first.imageSmoothingEnabled = false;
      for (const column of columns) {
        first.drawImage(
          rendered,
          column.start,
          crop.y,
          column.length,
          crop.height,
          column.offset,
          0,
          column.length,
          crop.height,
        );
      }
      rendered.width = 0;
      rendered.height = 0;
    }
    output.width = width;
    output.height = height;
    const ctx = canvasContext(output);
    ctx.imageSmoothingEnabled = false;
    if (format === 'image/jpeg') {
      ctx.fillStyle = '#fff';
      ctx.fillRect(0, 0, width, height);
    }
    if (intermediate) {
      for (const row of rows) {
        ctx.drawImage(
          intermediate,
          0,
          row.start - crop.y,
          width,
          row.length,
          0,
          row.offset,
          width,
          row.length,
        );
      }
    } else {
      for (const column of columns) {
        for (const row of rows) {
          ctx.drawImage(
            rendered,
            column.start,
            row.start,
            column.length,
            row.length,
            column.offset,
            row.offset,
            column.length,
            row.length,
          );
        }
      }
    }
    return output;
  } catch (cause) {
    output.width = 0;
    output.height = 0;
    throw cause;
  } finally {
    if (intermediate) {
      intermediate.width = 0;
      intermediate.height = 0;
    }
  }
}
