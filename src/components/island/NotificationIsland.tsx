import { X } from 'lucide-react';
import type { IslandNotification } from '../../stores/island-machine';
import { ZoneMark } from '../common/ZoneMark';

interface NotificationIslandProps {
  notification: IslandNotification;
  onDismiss: () => void;
  onOpen: () => void;
}

export function NotificationIsland({ notification, onDismiss, onOpen }: NotificationIslandProps) {
  return (
    <div className="notification-island" role="status">
      <button
        className="notification-content"
        onClick={onOpen}
        type="button"
        aria-label="Mở lịch sử thông báo"
      >
        <span className="notification-mark">
          <ZoneMark small />
        </span>
        <span className="notification-copy">
          <strong>{notification.title}</strong>
          {notification.body && <span>{notification.body}</span>}
        </span>
      </button>
      <button
        className="notification-dismiss"
        type="button"
        onClick={onDismiss}
        aria-label="Đóng thông báo"
      >
        <X size={14} />
      </button>
    </div>
  );
}
