import {
  ArrowUpRight,
  Circle,
  Copy,
  Crop,
  Download,
  Droplets,
  Eraser,
  Grid2X2,
  Hash,
  Highlighter,
  Image,
  MessageSquare,
  Minus,
  MousePointer2,
  Pencil,
  Pin,
  Redo2,
  Scissors,
  Smile,
  Square,
  Sun,
  Trash2,
  Type,
  Undo2,
  X,
  ZoomIn,
  type LucideIcon,
} from 'lucide-react';
import type { ReactNode } from 'react';
import type { EditorFormat } from './Editor';
import type { Annotation, AnnotationStyle, Rect, Tool } from './model';

const tools: { id: Tool; label: string; Icon: LucideIcon }[] = [
  { id: 'select', label: 'Chọn và chỉnh sửa', Icon: MousePointer2 },
  { id: 'crop', label: 'Chọn vùng chụp', Icon: Crop },
  { id: 'rect', label: 'Hình chữ nhật', Icon: Square },
  { id: 'ellipse', label: 'Hình elip', Icon: Circle },
  { id: 'line', label: 'Đường thẳng', Icon: Minus },
  { id: 'arrow', label: 'Mũi tên', Icon: ArrowUpRight },
  { id: 'freehand', label: 'Vẽ tự do', Icon: Pencil },
  { id: 'text', label: 'Văn bản', Icon: Type },
  { id: 'bubble', label: 'Bong bóng lời thoại', Icon: MessageSquare },
  { id: 'step', label: 'Số thứ tự', Icon: Hash },
  { id: 'image', label: 'Chèn ảnh', Icon: Image },
  { id: 'emoji', label: 'Emoji', Icon: Smile },
  { id: 'highlight', label: 'Tô sáng', Icon: Highlighter },
  { id: 'blur', label: 'Làm mờ', Icon: Droplets },
  { id: 'pixelate', label: 'Pixel hóa', Icon: Grid2X2 },
  { id: 'eraser', label: 'Tẩy chú thích', Icon: Eraser },
  { id: 'magnifier', label: 'Kính lúp', Icon: ZoomIn },
  { id: 'spotlight', label: 'Vùng nổi bật', Icon: Sun },
  { id: 'cutout', label: 'Cắt bỏ vùng', Icon: Scissors },
];

const fonts = ['Segoe UI', 'Arial', 'Georgia', 'Consolas', 'Times New Roman'];
const emojiPresets = ['🙂', '👍', '✅', '⚠️', '⭐', '💡', '❤️', '🎯', '❌'];
const textTools: Tool[] = ['text', 'bubble', 'step', 'emoji'];
const coloredTools: Tool[] = [
  'rect',
  'ellipse',
  'line',
  'arrow',
  'freehand',
  'text',
  'bubble',
  'step',
  'highlight',
  'magnifier',
];
const strokedTools: Tool[] = [
  'rect',
  'ellipse',
  'line',
  'arrow',
  'freehand',
  'bubble',
  'magnifier',
];
const amountTools: Tool[] = ['blur', 'pixelate', 'magnifier'];

export function ToolButton({
  label,
  children,
  pressed,
  disabled,
  onClick,
}: {
  label: string;
  children: ReactNode;
  pressed?: boolean;
  disabled?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      className="shot-icon-button"
      aria-label={label}
      aria-pressed={pressed}
      title={label}
      disabled={disabled}
      onClick={onClick}
    >
      {children}
    </button>
  );
}

export function NumberField({
  label,
  caption = label,
  value,
  min,
  max,
  onChange,
}: {
  label: string;
  caption?: string;
  value: number;
  min: number;
  max: number;
  onChange: (value: number) => void;
}) {
  const shown = Math.round(value);
  return (
    <label className="shot-field shot-field-number">
      <span>{caption}</span>
      <input
        key={shown}
        type="number"
        aria-label={label}
        defaultValue={shown}
        min={min}
        max={max}
        step={1}
        onBlur={(event) => {
          const next = event.currentTarget.valueAsNumber;
          const clamped = Number.isFinite(next)
            ? Math.max(min, Math.min(max, Math.round(next)))
            : shown;
          event.currentTarget.value = String(clamped);
          if (clamped !== shown) onChange(clamped);
        }}
        onKeyDown={(event) => {
          if (event.key === 'Enter') event.currentTarget.blur();
        }}
      />
    </label>
  );
}

type StyleUpdate = <K extends keyof AnnotationStyle>(key: K, value: AnnotationStyle[K]) => void;

interface EditorControlsProps {
  tool: Tool;
  selected?: Annotation;
  style: AnnotationStyle;
  text: string;
  assetName?: string;
  crop: Rect;
  frame: { width: number; height: number };
  format: EditorFormat;
  canUndo: boolean;
  canRedo: boolean;
  canExport: boolean;
  disabled: boolean;
  onTool: (tool: Tool) => void;
  onStyle: StyleUpdate;
  onText: (text: string) => void;
  onBox: (box: Rect) => void;
  onCrop: (box: Rect) => void;
  onImport: () => void;
  onUndo: () => void;
  onRedo: () => void;
  onDelete: () => void;
  onFormat: (format: EditorFormat) => void;
  onCopy: () => void;
  onSave: () => void;
  onPin: () => void;
  onCancel: () => void;
}

function BoxFields({
  box,
  frame,
  onChange,
  crop,
}: {
  box: Rect;
  frame: { width: number; height: number };
  onChange: (box: Rect) => void;
  crop?: boolean;
}) {
  return (
    <>
      <NumberField
        label={crop ? 'Vị trí ngang vùng chụp' : 'Vị trí ngang chú thích'}
        caption="X"
        value={box.x}
        min={crop ? 0 : -frame.width}
        max={frame.width - 1}
        onChange={(x) => onChange({ ...box, x })}
      />
      <NumberField
        label={crop ? 'Vị trí dọc vùng chụp' : 'Vị trí dọc chú thích'}
        caption="Y"
        value={box.y}
        min={crop ? 0 : -frame.height}
        max={frame.height - 1}
        onChange={(y) => onChange({ ...box, y })}
      />
      <NumberField
        label={crop ? 'Chiều rộng vùng chụp' : 'Chiều rộng chú thích'}
        caption="Rộng"
        value={box.width}
        min={1}
        max={crop ? frame.width - box.x : frame.width * 2}
        onChange={(width) => onChange({ ...box, width })}
      />
      <NumberField
        label={crop ? 'Chiều cao vùng chụp' : 'Chiều cao chú thích'}
        caption="Cao"
        value={box.height}
        min={1}
        max={crop ? frame.height - box.y : frame.height * 2}
        onChange={(height) => onChange({ ...box, height })}
      />
    </>
  );
}

export function EditorControls({
  tool,
  selected,
  style,
  text,
  assetName,
  crop,
  frame,
  format,
  canUndo,
  canRedo,
  canExport,
  disabled,
  onTool,
  onStyle,
  onText,
  onBox,
  onCrop,
  onImport,
  onUndo,
  onRedo,
  onDelete,
  onFormat,
  onCopy,
  onSave,
  onPin,
  onCancel,
}: EditorControlsProps) {
  const activeTool = selected?.tool ?? tool;
  const editingText = textTools.includes(activeTool);
  const label = tools.find(({ id }) => id === activeTool)?.label;
  const magnifier = activeTool === 'magnifier';
  const amountCaption =
    activeTool === 'blur' ? 'Bán kính' : magnifier ? 'Thu phóng (%)' : 'Ô pixel';
  return (
    <div className="shot-toolbar" aria-label="Thanh công cụ chỉnh sửa ảnh">
      <div className="shot-toolbar-tools" role="toolbar" aria-label="Công cụ chú thích">
        {tools.map(({ id, label, Icon }) => (
          <ToolButton
            key={id}
            label={label}
            pressed={id === tool}
            disabled={disabled}
            onClick={() => onTool(id)}
          >
            <Icon size={16} aria-hidden="true" />
          </ToolButton>
        ))}
        <span className="shot-toolbar-divider" aria-hidden="true" />
        <ToolButton label="Hoàn tác (Ctrl+Z)" disabled={disabled || !canUndo} onClick={onUndo}>
          <Undo2 size={16} aria-hidden="true" />
        </ToolButton>
        <ToolButton label="Làm lại (Ctrl+Y)" disabled={disabled || !canRedo} onClick={onRedo}>
          <Redo2 size={16} aria-hidden="true" />
        </ToolButton>
        <ToolButton
          label="Xóa chú thích đã chọn"
          disabled={disabled || !selected}
          onClick={onDelete}
        >
          <Trash2 size={15} aria-hidden="true" />
        </ToolButton>
      </div>
      <fieldset className="shot-inspector" disabled={disabled}>
        <legend className="shot-sr-only">Thuộc tính công cụ</legend>
        <span className="shot-inspector-title">{label}</span>
        {tool === 'crop' ? (
          <BoxFields box={crop} frame={frame} onChange={onCrop} crop />
        ) : (
          <>
            {coloredTools.includes(activeTool) && (
              <label className="shot-field">
                <span>Màu</span>
                <input
                  type="color"
                  aria-label="Màu chú thích"
                  value={style.color}
                  onChange={(event) => onStyle('color', event.target.value)}
                />
              </label>
            )}
            {strokedTools.includes(activeTool) && (
              <NumberField
                label="Độ dày nét"
                caption="Nét"
                value={style.strokeWidth}
                min={1}
                max={160}
                onChange={(value) => onStyle('strokeWidth', value)}
              />
            )}
            {activeTool !== 'select' && activeTool !== 'eraser' && activeTool !== 'cutout' && (
              <NumberField
                label="Độ trong suốt theo phần trăm"
                caption="Độ đậm"
                value={style.opacity * 100}
                min={0}
                max={100}
                onChange={(value) => onStyle('opacity', value / 100)}
              />
            )}
            {(activeTool === 'rect' || activeTool === 'ellipse' || activeTool === 'bubble') && (
              <label className="shot-field shot-check">
                <input
                  type="checkbox"
                  checked={style.fill}
                  onChange={(event) => onStyle('fill', event.target.checked)}
                />
                <span>Tô kín</span>
              </label>
            )}
            {editingText && (
              <>
                <label className="shot-field shot-field-text">
                  <span>Nội dung</span>
                  <input
                    key={selected?.id ?? activeTool}
                    type="text"
                    aria-label="Nội dung chú thích"
                    maxLength={500}
                    placeholder={
                      activeTool === 'emoji' ? '🙂' : activeTool === 'step' ? '1' : 'Nhập nội dung…'
                    }
                    value={text}
                    onChange={(event) => onText(event.target.value)}
                  />
                </label>
                {activeTool === 'emoji' && (
                  <label className="shot-field">
                    <span>Chọn nhanh</span>
                    <select
                      aria-label="Emoji có sẵn"
                      value={emojiPresets.includes(text) ? text : ''}
                      onChange={(event) => {
                        if (event.target.value) onText(event.target.value);
                      }}
                    >
                      <option value="" disabled>
                        Emoji…
                      </option>
                      {emojiPresets.map((emoji) => (
                        <option key={emoji}>{emoji}</option>
                      ))}
                    </select>
                  </label>
                )}
                <label className="shot-field">
                  <span>Font</span>
                  <select
                    aria-label="Font chữ"
                    value={style.fontFamily}
                    onChange={(event) => onStyle('fontFamily', event.target.value)}
                  >
                    {fonts.map((font) => (
                      <option key={font}>{font}</option>
                    ))}
                  </select>
                </label>
                <NumberField
                  label="Kích thước chữ"
                  caption="Chữ"
                  value={style.fontSize}
                  min={8}
                  max={240}
                  onChange={(value) => onStyle('fontSize', value)}
                />
              </>
            )}
            {amountTools.includes(activeTool) && (
              <NumberField
                label={amountCaption}
                value={
                  magnifier
                    ? Math.min(800, Math.max(125, (1 + style.amount / 12) * 100))
                    : style.amount
                }
                min={magnifier ? 125 : 1}
                max={magnifier ? 800 : 100}
                onChange={(value) =>
                  onStyle('amount', magnifier ? ((value - 100) / 100) * 12 : value)
                }
              />
            )}
            {activeTool === 'image' && (
              <button type="button" className="shot-button shot-button-small" onClick={onImport}>
                {assetName ?? 'Chọn ảnh…'}
              </button>
            )}
            {selected && <BoxFields box={selected.box} frame={frame} onChange={onBox} />}
            {activeTool === 'select' && !selected && (
              <span className="shot-inspector-hint">
                Bấm chú thích để sửa hoặc kéo để di chuyển.
              </span>
            )}
            {activeTool === 'eraser' && (
              <span className="shot-inspector-hint">Bấm hoặc kéo qua chú thích để xóa.</span>
            )}
            {activeTool === 'cutout' && (
              <>
                <label className="shot-field">
                  <span>Dải cắt</span>
                  <select
                    aria-label="Hướng cắt bỏ"
                    value={style.cutoutDirection ?? 'horizontal'}
                    onChange={(event) =>
                      onStyle(
                        'cutoutDirection',
                        event.target.value === 'vertical' ? 'vertical' : 'horizontal',
                      )
                    }
                  >
                    <option value="horizontal">Ngang</option>
                    <option value="vertical">Dọc</option>
                  </select>
                </label>
                <span className="shot-inspector-hint">
                  Kéo chọn dải cần bỏ. Zone ghép hai phần còn lại khi xuất.
                </span>
              </>
            )}
          </>
        )}
      </fieldset>
      <div className="shot-actions" role="group" aria-label="Xuất vùng chụp">
        <button
          type="button"
          className="shot-button"
          title="Sao chép vào clipboard (Enter hoặc Ctrl+C)"
          aria-keyshortcuts="Enter Control+C"
          disabled={disabled || !canExport}
          onClick={onCopy}
        >
          <Copy size={15} aria-hidden="true" /> Sao chép
        </button>
        <label className="shot-format">
          <span className="shot-sr-only">Định dạng xuất ảnh</span>
          <select
            value={format}
            disabled={disabled}
            onChange={(event) => onFormat(event.target.value === 'jpeg' ? 'jpeg' : 'png')}
          >
            <option value="png">PNG</option>
            <option value="jpeg">JPEG</option>
          </select>
        </label>
        <button
          type="button"
          className="shot-button shot-primary"
          title="Lưu ảnh (Ctrl+S)"
          aria-keyshortcuts="Control+S"
          disabled={disabled || !canExport}
          onClick={onSave}
        >
          <Download size={15} aria-hidden="true" /> Lưu ảnh
        </button>
        <button
          type="button"
          className="shot-button"
          disabled={disabled || !canExport}
          onClick={onPin}
        >
          <Pin size={15} aria-hidden="true" /> Ghim ảnh
        </button>
        <button
          type="button"
          className="shot-button shot-cancel"
          aria-label="Hủy trình chỉnh sửa"
          title="Hủy (Esc)"
          disabled={disabled}
          onClick={onCancel}
        >
          <X size={17} aria-hidden="true" />
        </button>
      </div>
    </div>
  );
}
