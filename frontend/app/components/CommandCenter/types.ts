export interface ItemCommand {
  id: string;
  icon: string;
  title: string;
  description: string;
  /** Navigation target; absent for items that run a callback instead. */
  path?: string;
  /** Plugin commands execute in place instead of navigating. */
  run?: () => void;
  section: string;
  globalIndex: number;
  shortcut?: string;
}
