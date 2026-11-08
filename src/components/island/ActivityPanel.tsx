import { motion } from 'motion/react';
import { Bell, CheckCheck, Trash2 } from 'lucide-react';
import type { IslandNotification } from '../../stores/island-machine';
import { contentVariants, staticContentVariants } from '../../animations/presets';

interface ActivityPanelProps {
  notifications: IslandNotification[];
  onClear: () => void;
  reducedMotion: boolean;
}

export function ActivityPanel({ notifications, onClear, reducedMotion }: ActivityPanelProps) {
  const variants = reducedMotion ? staticContentVariants : contentVariants;
  return (
    <motion.div
      className="activity-panel"
      variants={variants}
      initial="hidden"
      animate="visible"
      exit="exit"
    >
      <div className="activity-heading">
        <div>
          <h1>Thông báo của bạn</h1>
          <p>Những cập nhật nhỏ, không làm ngắt mạch công việc.</p>
        </div>
        {notifications.length > 0 && (
          <button className="text-button" type="button" onClick={onClear}>
            <Trash2 size={13} /> Xóa hết
          </button>
        )}
      </div>
      {notifications.length === 0 ? (
        <div className="empty-activity">
          <span>
            <CheckCheck size={26} strokeWidth={1.2} />
          </span>
          <strong>Mọi thứ đã gọn gàng.</strong>
          <p>
            Thông báo sẽ xuất hiện ở đây.
            <br />
            Lúc này, cứ dành chỗ cho điều bạn đang làm.
          </p>
        </div>
      ) : (
        <div className="activity-list">
          {notifications.map((item) => (
            <motion.div key={item.id} className="activity-item" variants={variants}>
              <span className="activity-icon">
                <Bell size={14} />
              </span>
              <div>
                <strong>{item.title}</strong>
                {item.body && <p>{item.body}</p>}
              </div>
            </motion.div>
          ))}
        </div>
      )}
      <p className="activity-footnote">Chỉ trong phiên này · Tối đa 30 thông báo gần nhất</p>
    </motion.div>
  );
}
