export { WindowFrame, type WindowFrameProps } from "./components/WindowFrame";
export { Apptitlebar } from "./components/AppTitlebar";
export { useWindowFlat } from "./hook/useWindowFlat";
export { useClickThroughMargin } from "./hook/useClickthrough";
export {
  getResizeHandles,
  isInsideHitArea,
  type FrameMetrics,
  type ResizeDirection,
} from "./utils/geometry";
export * from "./components/Sidebar";
export { Icon, type IconType, type IconProps } from "./components/Icon";
export { usePressScale, type PressScaleOptions } from "./hook/usePressScale";
export {
  PageHeaderProvider,
  usePageHeader,
  usePageHeaderCrumbs,
  type Crumb,
  type CrumbMenu,
} from "./lib/page-header";
export { useElementHeight } from "./hook/useElementHeight";
export { mergeRefs } from "./utils/merge-refs";
export { AppContextMenu } from "./components/AppContextMenu";
export { useContextMenu } from "./hook/useContextMenu";
export type {
  ContextMenuAction,
  ContextMenuEntry,
  ContextMenuGroup,
  ContextMenuProvider,
  ContextMenuRequest,
  ContextMenuSubmenu,
} from "./lib/context-menu/types";
export { shortcut } from "./utils/keys";
export { PressButton, type PressButtonProps } from "./components/PressButton";
export { SettingsDialogProvider, useSettingsDialog } from "./lib/settings-dialog";
export { AddAgentDialogProvider, useAddAgentDialog, type AddAgentRequest } from "./lib/add-agent-dialog";
export { notifyTeamChanged, useTeamChanged } from "./lib/team-changes";
export { useFileDrop } from "./hook/useFileDrop";
export { DropOverlay } from "./components/DropOverlay";
export { TagEditor } from "./components/TagEditor";
export { SetupSection } from "./components/SetupSection";
export { NotVerifiedDialog, type OverrideOutcome } from "./components/NotVerifiedDialog";
