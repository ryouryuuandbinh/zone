import { create } from 'zustand';
import {
  initialIslandState,
  reduceIsland,
  type IslandEvent,
  type IslandState,
} from './island-machine';

interface IslandStore extends IslandState {
  dispatch: (event: IslandEvent) => void;
}

export const useIslandStore = create<IslandStore>((set) => ({
  ...initialIslandState,
  dispatch: (event) => set((state) => reduceIsland(state, event)),
}));
