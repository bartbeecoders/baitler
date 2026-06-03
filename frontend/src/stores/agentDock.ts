import { create } from 'zustand';
import { persist } from 'zustand/middleware';

export const DOCK_MIN_WIDTH = 320;
export const DOCK_MAX_WIDTH = 960;
export const DOCK_DEFAULT_WIDTH = 384; // = w-96

/** Clamp a width to sane bounds so a stale/garbage persisted value can't break
 * the layout. */
export function clampDockWidth(w: number): number {
  return Math.max(DOCK_MIN_WIDTH, Math.min(DOCK_MAX_WIDTH, Math.round(w)));
}

interface AgentDockState {
  /** Whether the right-hand agent pane is open. */
  open: boolean;
  /** Pane width in px (clamped). */
  width: number;
  toggle: () => void;
  setOpen: (open: boolean) => void;
  setWidth: (width: number) => void;
}

/** Open/closed state + width of the docked Agent pane, persisted across sessions. */
export const useAgentDock = create<AgentDockState>()(
  persist(
    (set, get) => ({
      open: false,
      width: DOCK_DEFAULT_WIDTH,
      toggle: () => set({ open: !get().open }),
      setOpen: (open) => set({ open }),
      setWidth: (width) => set({ width: clampDockWidth(width) }),
    }),
    { name: 'baitler-agent-dock' },
  ),
);
