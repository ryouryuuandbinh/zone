export type IslandMode = 'idle' | 'hovered' | 'expanded' | 'notification';
export type IslandTab =
  'overview' | 'activity' | 'clipboard' | 'notes' | 'bookmarks' | 'screenshot';

export interface IslandNotification {
  id: string;
  title: string;
  body?: string;
}

export interface IslandState {
  mode: IslandMode;
  tab: IslandTab;
  pointerInside: boolean;
  pinned: boolean;
  dismissOnLeave: boolean;
  interactionLocks: string[];
  notification: IslandNotification | null;
  pending: IslandNotification[];
  history: IslandNotification[];
}

export const initialIslandState: IslandState = {
  mode: 'idle',
  tab: 'overview',
  pointerInside: false,
  pinned: false,
  dismissOnLeave: false,
  interactionLocks: [],
  notification: null,
  pending: [],
  history: [],
};

export type IslandEvent =
  | { type: 'POINTER_ENTER' }
  | { type: 'POINTER_LEAVE' }
  | { type: 'OPEN' }
  | { type: 'CLOSE' }
  | { type: 'TOGGLE' }
  | { type: 'PIN' }
  | { type: 'SET_TAB'; tab: IslandTab }
  | { type: 'OPEN_PANEL'; tab: IslandTab }
  | { type: 'LOCK_INTERACTION'; id: string }
  | { type: 'UNLOCK_INTERACTION'; id: string }
  | { type: 'NOTIFY'; notification: IslandNotification }
  | { type: 'DISMISS_NOTIFICATION' }
  | { type: 'CLEAR_HISTORY' };

function collapse(state: IslandState): IslandState {
  const [next, ...remaining] = state.pending;
  return {
    ...state,
    mode: next ? 'notification' : 'idle',
    pinned: false,
    dismissOnLeave: false,
    interactionLocks: [],
    notification: next ?? null,
    pending: remaining,
  };
}

/** User interaction owns the surface; notifications wait instead of replacing it. */
export function reduceIsland(state: IslandState, event: IslandEvent): IslandState {
  switch (event.type) {
    case 'POINTER_ENTER':
      return state.pointerInside
        ? state
        : {
            ...state,
            pointerInside: true,
            dismissOnLeave: state.mode === 'expanded' || state.dismissOnLeave,
            mode: state.mode === 'idle' ? 'hovered' : state.mode,
          };
    case 'POINTER_LEAVE':
      return !state.pointerInside
        ? state
        : { ...state, pointerInside: false, mode: state.mode === 'hovered' ? 'idle' : state.mode };
    case 'OPEN':
      return state.mode === 'expanded'
        ? state
        : { ...state, mode: 'expanded', dismissOnLeave: state.pointerInside, notification: null };
    case 'CLOSE':
      return state.mode === 'expanded' ? collapse(state) : state;
    case 'TOGGLE':
      return state.mode === 'expanded'
        ? collapse(state)
        : { ...state, mode: 'expanded', dismissOnLeave: state.pointerInside, notification: null };
    case 'PIN':
      return state.mode === 'expanded' ? { ...state, pinned: !state.pinned } : state;
    case 'OPEN_PANEL':
      return {
        ...state,
        mode: 'expanded',
        tab: event.tab,
        dismissOnLeave: false,
        notification: null,
        pending: event.tab === 'activity' ? [] : state.pending,
      };
    case 'LOCK_INTERACTION':
      return state.interactionLocks.includes(event.id)
        ? state
        : { ...state, interactionLocks: [...state.interactionLocks, event.id] };
    case 'UNLOCK_INTERACTION':
      return state.interactionLocks.includes(event.id)
        ? { ...state, interactionLocks: state.interactionLocks.filter((id) => id !== event.id) }
        : state;
    case 'SET_TAB':
      return { ...state, tab: event.tab, pending: event.tab === 'activity' ? [] : state.pending };
    case 'NOTIFY': {
      if (state.history.some((item) => item.id === event.notification.id)) return state;
      const history = [event.notification, ...state.history].slice(0, 30);
      if (state.mode === 'expanded' || state.mode === 'notification') {
        return { ...state, history, pending: [...state.pending, event.notification].slice(-8) };
      }
      return { ...state, history, mode: 'notification', notification: event.notification };
    }
    case 'DISMISS_NOTIFICATION':
      return state.mode === 'notification' ? collapse(state) : state;
    case 'CLEAR_HISTORY':
      return { ...state, history: [], pending: [] };
  }
}
