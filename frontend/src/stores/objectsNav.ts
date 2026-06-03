import { create } from 'zustand';
import { persist } from 'zustand/middleware';

interface ObjectsNavState {
  /** Whether the "Objects" group is expanded. */
  groupOpen: boolean;
  /** The single expanded object type (accordion behaviour), or null. */
  openType: string | null;
  toggleGroup: () => void;
  setGroupOpen: (open: boolean) => void;
  /** Expand a type (collapsing any other); pass the same key again to collapse. */
  toggleType: (key: string) => void;
  setOpenType: (key: string | null) => void;
}

/** Sidebar "Objects" accordion expansion state — persisted so it survives reloads. */
export const useObjectsNav = create<ObjectsNavState>()(
  persist(
    (set) => ({
      groupOpen: true,
      openType: 'documents',
      toggleGroup: () => set((s) => ({ groupOpen: !s.groupOpen })),
      setGroupOpen: (open) => set({ groupOpen: open }),
      toggleType: (key) => set((s) => ({ openType: s.openType === key ? null : key })),
      setOpenType: (key) => set({ openType: key }),
    }),
    { name: 'baitler-objects-nav' },
  ),
);
