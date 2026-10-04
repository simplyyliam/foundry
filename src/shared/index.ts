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
