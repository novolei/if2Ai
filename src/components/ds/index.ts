// Paico Design System Components
// Reusable building blocks from the Jade Mist Teal design system.
// These components use CSS variables from globals.css and are NOT yet
// wired into existing business logic — they serve as building blocks.

export { default as StatusTag } from './StatusTag';
export type { StatusTagProps, StatusVariant } from './StatusTag';

export { default as Sidebar } from './Sidebar';
export type { SidebarProps, NavItemDef } from './Sidebar';

export { default as SidebarItem } from './SidebarItem';
export type { SidebarItemProps } from './SidebarItem';

export { default as MessageItem } from './MessageItem';
export type { MessageItemProps } from './MessageItem';

export { default as ChatTimeline } from './ChatTimeline';
export type { ChatTimelineProps, ChatMessage } from './ChatTimeline';

export { default as FileListItem, type FileEntry } from './FileListItem';
export type { FileListItemProps } from './FileListItem';

export { default as InspectorPanel } from './InspectorPanel';
export type { InspectorPanelProps } from './InspectorPanel';

export { default as InputComposer } from './InputComposer';
export type { InputComposerProps } from './InputComposer';

export { default as Button } from './Button';
export type { ButtonProps, ButtonVariant, ButtonSize } from './Button';
