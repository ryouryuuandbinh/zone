import { describe, expect, it } from 'vitest';
import { initialIslandState, reduceIsland, type IslandState } from '../src/stores/island-machine';

const notification = (id: string) => ({ id, title: `Event ${id}`, body: 'Local notification' });

describe('Island interaction arbitration', () => {
  it('opens a requested module directly and preserves queued notifications', () => {
    let state = reduceIsland(initialIslandState, { type: 'OPEN' });
    state = reduceIsland(state, { type: 'NOTIFY', notification: notification('module-event') });
    state = reduceIsland(state, { type: 'OPEN_PANEL', tab: 'notes' });
    expect(state).toMatchObject({ mode: 'expanded', tab: 'notes', dismissOnLeave: false });
    expect(state.pending.map((item) => item.id)).toEqual(['module-event']);
  });

  it('keeps separate editor and native-picker interaction locks', () => {
    let state = reduceIsland(initialIslandState, { type: 'OPEN_PANEL', tab: 'bookmarks' });
    state = reduceIsland(state, { type: 'LOCK_INTERACTION', id: 'editor' });
    state = reduceIsland(state, { type: 'LOCK_INTERACTION', id: 'picker' });
    expect(reduceIsland(state, { type: 'LOCK_INTERACTION', id: 'picker' })).toBe(state);
    state = reduceIsland(state, { type: 'UNLOCK_INTERACTION', id: 'picker' });
    expect(state.interactionLocks).toEqual(['editor']);
    state = reduceIsland(state, { type: 'CLOSE' });
    expect(state.mode).toBe('idle');
    expect(state.interactionLocks).toEqual([]);
  });
  it('defers incoming notifications without changing the active panel or pin', () => {
    let state = reduceIsland(initialIslandState, { type: 'OPEN' });
    state = reduceIsland(state, { type: 'SET_TAB', tab: 'activity' });
    state = reduceIsland(state, { type: 'PIN' });
    state = reduceIsland(state, { type: 'NOTIFY', notification: notification('1') });
    expect(state).toMatchObject({ mode: 'expanded', tab: 'activity', pinned: true });
    expect(state.pending.map((item) => item.id)).toEqual(['1']);
    expect(state.notification).toBeNull();
  });

  it('delivers deferred notifications when interaction closes, in arrival order', () => {
    let state = reduceIsland(initialIslandState, { type: 'OPEN' });
    for (const id of ['first', 'second']) {
      state = reduceIsland(state, { type: 'NOTIFY', notification: notification(id) });
    }
    state = reduceIsland(state, { type: 'CLOSE' });
    expect(state.mode).toBe('notification');
    expect(state.notification?.id).toBe('first');
    state = reduceIsland(state, { type: 'DISMISS_NOTIFICATION' });
    expect(state.notification?.id).toBe('second');
    state = reduceIsland(state, { type: 'DISMISS_NOTIFICATION' });
    expect(state.mode).toBe('idle');
    expect(state.notification).toBeNull();
  });

  it('never replaces a visible notification when another arrives', () => {
    let state = reduceIsland(initialIslandState, {
      type: 'NOTIFY',
      notification: notification('1'),
    });
    state = reduceIsland(state, { type: 'NOTIFY', notification: notification('2') });
    expect(state.notification?.id).toBe('1');
    expect(state.pending[0]?.id).toBe('2');
  });

  it('bounds pending work and history during an event burst', () => {
    let state: IslandState = reduceIsland(initialIslandState, { type: 'OPEN' });
    for (let index = 0; index < 100; index += 1) {
      state = reduceIsland(state, { type: 'NOTIFY', notification: notification(String(index)) });
    }
    expect(state.mode).toBe('expanded');
    expect(state.pending).toHaveLength(8);
    expect(state.history).toHaveLength(30);
    expect(state.pending[0]?.id).toBe('92');
  });

  it('deduplicates repeated delivery of the same event', () => {
    const state = reduceIsland(initialIslandState, {
      type: 'NOTIFY',
      notification: notification('1'),
    });
    const duplicate = reduceIsland(state, { type: 'NOTIFY', notification: notification('1') });
    expect(duplicate).toBe(state);
  });

  it('keeps an expanded panel open through pointer enter and leave', () => {
    let state = reduceIsland(initialIslandState, { type: 'OPEN' });
    state = reduceIsland(state, { type: 'POINTER_ENTER' });
    state = reduceIsland(state, { type: 'POINTER_LEAVE' });
    expect(state.mode).toBe('expanded');
    expect(state.pointerInside).toBe(false);
  });

  it('handles rapid reversal without leaving stale pin or notification state', () => {
    let state = initialIslandState;
    for (let index = 0; index < 200; index += 1) {
      state = reduceIsland(state, { type: 'TOGGLE' });
    }
    expect(state.mode).toBe('idle');
    expect(state.notification).toBeNull();
    expect(state.pinned).toBe(false);
  });

  it('avoids no-op renders on duplicate pointer events', () => {
    const entered = reduceIsland(initialIslandState, { type: 'POINTER_ENTER' });
    expect(reduceIsland(entered, { type: 'POINTER_ENTER' })).toBe(entered);
    const left = reduceIsland(entered, { type: 'POINTER_LEAVE' });
    expect(reduceIsland(left, { type: 'POINTER_LEAVE' })).toBe(left);
    expect(left.mode).toBe('idle');
  });

  it('keeps a keyboard-opened panel available until pointer interaction begins', () => {
    let state = reduceIsland(initialIslandState, { type: 'TOGGLE' });
    expect(state.mode).toBe('expanded');
    expect(state.dismissOnLeave).toBe(false);
    state = reduceIsland(state, { type: 'POINTER_ENTER' });
    state = reduceIsland(state, { type: 'POINTER_LEAVE' });
    expect(state.dismissOnLeave).toBe(true);
    expect(state.mode).toBe('expanded');
    state = reduceIsland(state, { type: 'CLOSE' });
    expect(state.dismissOnLeave).toBe(false);
  });

  it('arms hover-opened panels for leave-based dismissal', () => {
    let state = reduceIsland(initialIslandState, { type: 'POINTER_ENTER' });
    state = reduceIsland(state, { type: 'OPEN' });
    expect(state.dismissOnLeave).toBe(true);
  });
});
