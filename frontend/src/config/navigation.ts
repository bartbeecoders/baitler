import {
  LayoutDashboard,
  FolderOpen,
  Lightbulb,
  BarChart3,
  FileText,
  Sparkles,
  type LucideIcon,
} from 'lucide-react';

export interface NavItem {
  label: string;
  /** Route path. */
  path: string;
  icon: LucideIcon;
  /** Short blurb shown on the dashboard feature cards. */
  description: string;
  /** The plan.md phase that delivers this feature (`null` = available now). */
  phase: number | null;
}

/** Primary navigation — drives both the sidebar and the dashboard cards. */
export const navItems: NavItem[] = [
  {
    label: 'Dashboard',
    path: '/',
    icon: LayoutDashboard,
    description: 'Your base portal — everything in one place.',
    phase: null,
  },
  {
    label: 'Files',
    path: '/files',
    icon: FolderOpen,
    description: 'Store, organize, and preview your files.',
    phase: 4,
  },
  {
    label: 'Ideas',
    path: '/ideas',
    icon: Lightbulb,
    description: 'Capture, tag, and link your ideas.',
    phase: 5,
  },
  {
    label: 'Documents',
    path: '/editor',
    icon: FileText,
    description: 'Write HTML & Markdown; export to PDF and Office.',
    phase: 7,
  },
  {
    label: 'Analytics',
    path: '/analytics',
    icon: BarChart3,
    description: 'Visualize your data, activity, and usage.',
    phase: 8,
  },
  {
    label: 'AI',
    path: '/ai',
    icon: Sparkles,
    description: 'Analyze and chat with your data across providers.',
    phase: 6,
  },
];

/** Feature cards on the base portal (everything except the dashboard itself). */
export const featureItems: NavItem[] = navItems.filter((item) => item.path !== '/');
