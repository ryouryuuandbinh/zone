import { motion } from 'motion/react';
import {
  ArrowUpRight,
  Bell,
  Bookmark,
  Camera,
  Check,
  ClipboardList,
  NotebookPen,
  ShieldCheck,
  SlidersHorizontal,
} from 'lucide-react';
import { contentVariants, staticContentVariants } from '../../animations/presets';
import { nativeAvailable } from '../../services/native';
import { useIslandStore } from '../../stores/island-store';

interface OverviewPanelProps {
  onNotify: () => void;
  onSettings: () => void;
  notificationQueued: boolean;
  reducedMotion: boolean;
}
const actions = [
  {
    tab: 'clipboard',
    title: 'Clipboard',
    detail: 'Tìm lại những mục vừa chép',
    icon: ClipboardList,
  },
  { tab: 'notes', title: 'Ghi chú', detail: 'Ghi nhanh điều cần nhớ', icon: NotebookPen },
  { tab: 'bookmarks', title: 'Dấu trang', detail: 'Mở tệp và thư mục quen', icon: Bookmark },
] as const;

export function OverviewPanel({
  onNotify,
  onSettings,
  notificationQueued,
  reducedMotion,
}: OverviewPanelProps) {
  const variants = reducedMotion ? staticContentVariants : contentVariants;
  const dispatch = useIslandStore((state) => state.dispatch);
  return (
    <motion.div
      className="overview-panel"
      variants={variants}
      initial="hidden"
      animate="visible"
      exit="exit"
    >
      <motion.div className="island-greeting" variants={variants}>
        <div className="eyebrow">
          <span className="status-dot" />
          {nativeAvailable ? 'GỌN KHÔNG GIAN. NHẸ TÂM TRÍ.' : 'BẢN XEM GIAO DIỆN'}
        </div>
        <h1>
          Một góc nhỏ.
          <br />
          <span>Cho ngày nhẹ hơn.</span>
        </h1>
        <p>Một không gian riêng, luôn ở ngay phía trên.</p>
      </motion.div>
      <motion.div className="quick-actions quick-actions-productivity" variants={variants}>
        {actions.map(({ tab, title, detail, icon: Icon }) => (
          <button
            type="button"
            className="quick-action productivity-launch"
            key={tab}
            onClick={() => dispatch({ type: 'SET_TAB', tab })}
          >
            <span className="action-icon">
              <Icon size={18} />
            </span>
            <span className="action-copy">
              <strong>{title}</strong>
              <span>{detail}</span>
            </span>
            <ArrowUpRight size={15} className="action-arrow" />
          </button>
        ))}
      </motion.div>
      <motion.div className="overview-bottom" variants={variants}>
        <span>
          <ShieldCheck size={11} />
          Trên thiết bị của bạn
        </span>
        <div>
          <button type="button" onClick={() => dispatch({ type: 'SET_TAB', tab: 'screenshot' })}>
            <Camera size={11} />
            Chụp màn hình
          </button>
          <button
            type="button"
            onClick={onNotify}
            disabled={!nativeAvailable}
            title="Gửi một thông báo thử"
          >
            {notificationQueued ? <Check size={11} /> : <Bell size={11} />}
            {notificationQueued ? 'Đã xếp hàng' : 'Thử thông báo'}
          </button>
          <button type="button" onClick={onSettings} disabled={!nativeAvailable}>
            <SlidersHorizontal size={11} />
            Cài đặt
          </button>
        </div>
      </motion.div>
    </motion.div>
  );
}
