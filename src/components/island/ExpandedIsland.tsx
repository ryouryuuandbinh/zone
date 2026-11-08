import { AnimatePresence, motion } from 'motion/react';
import {
  Bell,
  Bookmark,
  Camera,
  ChevronUp,
  ClipboardList,
  Home,
  NotebookPen,
  Pin,
  Settings2,
} from 'lucide-react';
import { useIslandStore } from '../../stores/island-store';
import { usePreferencesStore } from '../../stores/preferences-store';
import { native } from '../../services/native';
import type { IslandTab } from '../../stores/island-machine';
import { ZoneMark } from '../common/ZoneMark';
import { IconButton } from '../common/IconButton';
import { OverviewPanel } from './OverviewPanel';
import { ActivityPanel } from './ActivityPanel';
import { ClipboardPanel } from '../../features/clipboard/ClipboardPanel';
import { NotesPanel } from '../../features/notes/NotesPanel';
import { BookmarksPanel } from '../../features/bookmarks/BookmarksPanel';
import { ScreenshotPanel } from '../../features/screenshot/ScreenshotPanel';

const tabs = [
  { id: 'overview', label: 'Tổng quan', icon: Home },
  { id: 'clipboard', label: 'Clipboard', icon: ClipboardList },
  { id: 'notes', label: 'Ghi chú', icon: NotebookPen },
  { id: 'bookmarks', label: 'Dấu trang', icon: Bookmark },
  { id: 'screenshot', label: 'Chụp ảnh', icon: Camera },
  { id: 'activity', label: 'Thông báo', icon: Bell },
] as const;

export function ExpandedIsland({ reducedMotion }: { reducedMotion: boolean }) {
  const tab = useIslandStore((state) => state.tab);
  const pinned = useIslandStore((state) => state.pinned);
  const history = useIslandStore((state) => state.history);
  const pending = useIslandStore((state) => state.pending);
  const dispatch = useIslandStore((state) => state.dispatch);
  const preferences = usePreferencesStore((state) => state.preferences);
  const reportError = usePreferencesStore((state) => state.reportError);
  const error = usePreferencesStore((state) => state.error);
  const settings = () => {
    void native
      .showSettings()
      .then(() => dispatch({ type: 'CLOSE' }))
      .catch(reportError);
  };
  const notify = () => {
    void native
      .notify(
        'Một lời nhắc từ Zone',
        'Đây là thông báo thử. Nội dung bạn đang xem vẫn được giữ nguyên.',
      )
      .catch(reportError);
  };
  const shortcuts: Partial<Record<IslandTab, string>> = {
    clipboard: preferences.clipboardShortcut,
    notes: preferences.notesShortcut,
    bookmarks: preferences.bookmarksShortcut,
  };
  const shortcutLabel = (shortcuts[tab] ?? preferences.shortcut)
    .replace('CommandOrControl', 'Ctrl')
    .replaceAll('+', ' + ');
  const feature = ['clipboard', 'notes', 'bookmarks', 'screenshot'].includes(tab);
  return (
    <div className={`expanded-island${feature ? ' expanded-productivity' : ''}`}>
      <header className="island-header">
        <span className="expanded-brand">
          <ZoneMark small />
          <span>
            zone<span className="brand-period">.</span>
          </span>
        </span>
        <motion.nav layout className="island-tabs" aria-label="Các bảng điều khiển">
          {tabs.map(({ id, label, icon: Icon }) => (
            <button
              key={id}
              type="button"
              className={tab === id ? 'selected' : ''}
              title={label}
              aria-label={label}
              aria-current={tab === id ? 'page' : undefined}
              onClick={() => dispatch({ type: 'SET_TAB', tab: id })}
            >
              {tab === id && (
                <motion.span
                  layoutId="active-tab"
                  className="active-tab"
                  transition={{ type: 'spring', stiffness: 350, damping: 30 }}
                />
              )}
              <span>
                <Icon size={13} strokeWidth={1.8} />
                {tab === id && <span className="tab-caption">{label}</span>}
                {id === 'activity' && pending.length > 0 && (
                  <i className="tab-badge">{pending.length}</i>
                )}
              </span>
            </button>
          ))}
        </motion.nav>
        <div className="header-actions">
          <IconButton
            label={pinned ? 'Bỏ ghim bảng điều khiển' : 'Giữ bảng điều khiển mở'}
            onClick={() => dispatch({ type: 'PIN' })}
            active={pinned}
          >
            <Pin size={14} />
          </IconButton>
          <IconButton label="Mở cài đặt" onClick={settings}>
            <Settings2 size={15} />
          </IconButton>
          <IconButton label="Thu gọn Zone" onClick={() => dispatch({ type: 'CLOSE' })}>
            <ChevronUp size={16} />
          </IconButton>
        </div>
      </header>
      <div className="island-panel-slot">
        <AnimatePresence mode="wait" initial={false}>
          <motion.div
            key={tab}
            className="island-panel-presence"
            initial={{ opacity: reducedMotion ? 1 : 0, y: reducedMotion ? 0 : 3 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: reducedMotion ? 1 : 0, y: reducedMotion ? 0 : -2 }}
            transition={{ duration: reducedMotion ? 0 : 0.12 }}
          >
            {tab === 'overview' ? (
              <OverviewPanel
                onNotify={notify}
                onSettings={settings}
                notificationQueued={pending.length > 0}
                reducedMotion={reducedMotion}
              />
            ) : tab === 'activity' ? (
              <ActivityPanel
                notifications={history}
                onClear={() => dispatch({ type: 'CLEAR_HISTORY' })}
                reducedMotion={reducedMotion}
              />
            ) : tab === 'clipboard' ? (
              <ClipboardPanel />
            ) : tab === 'notes' ? (
              <NotesPanel />
            ) : tab === 'screenshot' ? (
              <ScreenshotPanel />
            ) : (
              <BookmarksPanel />
            )}
          </motion.div>
        </AnimatePresence>
      </div>
      {error && (
        <div className="island-error" role="alert" title={error}>
          Zone cần được kiểm tra. Mở Cài đặt để xem chi tiết.
        </div>
      )}
      <motion.footer
        layout="position"
        className="island-footer"
        transition={{ type: 'spring', stiffness: 260, damping: 28, mass: 1 }}
      >
        <span>
          <span className="footer-glyph">Z</span> ZONE
        </span>
        <span className="shortcut-hint">
          {pinned ? 'Đang ghim mở' : shortcutLabel}
          <kbd>esc</kbd>
        </span>
      </motion.footer>
    </div>
  );
}
