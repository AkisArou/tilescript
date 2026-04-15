export interface LayoutBaseProps {
  id?: string;
  class?: string;
}

export type LayoutRenderable =
  | LayoutNode
  | string
  | number
  | boolean
  | null
  | undefined
  | LayoutRenderable[];
export type LayoutChildren = LayoutRenderable;

export interface LayoutComponentProps {
  children?: LayoutChildren;
}

export interface WorkspaceProps extends LayoutBaseProps {}

export interface GroupProps extends LayoutBaseProps {}

export interface SlotProps extends LayoutBaseProps {
  take?: number;
}

export interface WindowProps extends LayoutBaseProps {
  match?: string;
}

export type LayoutNodeChild = LayoutNode | null;
export type WorkspaceChild = LayoutNodeChild;
export type GroupChild = LayoutNodeChild;

export interface WorkspaceNode {
  type: "workspace";
  props?: WorkspaceProps;
  children?: WorkspaceChild[];
}

export interface GroupNode {
  type: "group";
  props?: GroupProps;
  children?: GroupChild[];
}

export interface SlotNode {
  type: "slot";
  props?: SlotProps;
  children?: never;
}

export interface WindowNode {
  type: "window";
  props?: WindowProps;
  children?: never;
}

export type LayoutNode = WorkspaceNode | GroupNode | SlotNode | WindowNode;
export type LayoutChild = LayoutNode | null;

export interface LayoutWindow {
  id: string;
  app_id?: string | null;
  title?: string | null;
  class?: string | null;
  instance?: string | null;
  role?: string | null;
  shell?: string | null;
  window_type?: string | null;
  floating?: boolean;
  fullscreen?: boolean;
  focused?: boolean;
}

export interface LayoutAdjustmentState {
  splitWeightsByNodeId?: Record<string, number[]>;
}

export interface LayoutState {
  prototype?: boolean;
  lastAction?: string;
  focusedWindowId?: string | null;
  currentOutputId?: string | null;
  currentWorkspaceId?: string | null;
  visibleWindowIds?: string[];
  workspaceNames?: string[];
  selectedLayoutName?: string | null;
  layoutAdjustments?: LayoutAdjustmentState;
  [key: string]: unknown;
}

export interface LayoutContext {
  monitor: {
    name: string;
    width: number;
    height: number;
    scale?: number;
  };
  workspace: {
    name: string;
    workspaces?: string[];
    windowCount: number;
  };
  windows: LayoutWindow[];
  state?: LayoutState;
}

export type LayoutFn = (ctx: LayoutContext) => LayoutRenderable;
