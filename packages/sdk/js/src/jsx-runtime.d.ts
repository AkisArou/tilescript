import type {
  GroupChild,
  GroupProps,
  GroupNode,
  LayoutNode,
  SlotNode,
  SlotProps,
  WindowNode,
  WorkspaceChild,
  WorkspaceNode,
  WindowProps,
  WorkspaceProps,
} from "./layout";

type Component<Props = Record<string, unknown>, Result = unknown> = (props: Props) => Result;
type JSXWorkspaceChildren = WorkspaceChild | WorkspaceChild[];
type JSXGroupChildren = GroupChild | GroupChild[];
type WorkspaceIntrinsicProps = WorkspaceProps & { children?: JSXWorkspaceChildren };
type GroupIntrinsicProps = GroupProps & { children?: JSXGroupChildren };
type SlotIntrinsicProps = SlotProps & { children?: never };
type WindowIntrinsicProps = WindowProps & { children?: never };

declare global {
  const Fragment: unique symbol;
  function sp(type: "workspace", props: WorkspaceIntrinsicProps | null, ...children: unknown[]): WorkspaceNode;
  function sp(type: "group", props: GroupIntrinsicProps | null, ...children: unknown[]): GroupNode;
  function sp(type: "slot", props: SlotIntrinsicProps | null, ...children: unknown[]): SlotNode;
  function sp(type: "window", props: WindowIntrinsicProps | null, ...children: unknown[]): WindowNode;
  function sp<Props, Result>(
    type: Component<Props, Result>,
    props: Props | null,
    ...children: unknown[]
  ): Result;
  function sp(
    type: typeof Fragment,
    props: Record<string, unknown> | null,
    ...children: unknown[]
  ): unknown;

  namespace JSX {
    type Element = LayoutNode;

    interface ElementChildrenAttribute {
      children: {};
    }

    interface IntrinsicAttributes {
      key?: string | number;
    }

    interface IntrinsicClassAttributes<T> {
      key?: string | number;
    }

    interface IntrinsicElements {
      workspace: WorkspaceIntrinsicProps;
      group: GroupIntrinsicProps;
      slot: SlotIntrinsicProps;
      window: WindowIntrinsicProps;
    }

    type LibraryManagedAttributes<C, P> = P;
  }
}

export declare function sp(type: "workspace", props: WorkspaceIntrinsicProps | null, ...children: unknown[]): WorkspaceNode;
export declare function sp(type: "group", props: GroupIntrinsicProps | null, ...children: unknown[]): GroupNode;
export declare function sp(type: "slot", props: SlotIntrinsicProps | null, ...children: unknown[]): SlotNode;
export declare function sp(type: "window", props: WindowIntrinsicProps | null, ...children: unknown[]): WindowNode;
export declare function sp<Props, Result>(
  type: Component<Props, Result>,
  props: Props | null,
  ...children: unknown[]
): Result;
export declare function sp(
  type: typeof Fragment,
  props: Record<string, unknown> | null,
  ...children: unknown[]
): unknown;
export declare function jsx(type: "workspace", props: WorkspaceIntrinsicProps | null, key?: unknown): WorkspaceNode;
export declare function jsx(type: "group", props: GroupIntrinsicProps | null, key?: unknown): GroupNode;
export declare function jsx(type: "slot", props: SlotIntrinsicProps | null, key?: unknown): SlotNode;
export declare function jsx(type: "window", props: WindowIntrinsicProps | null, key?: unknown): WindowNode;
export declare function jsx<Props, Result>(
  type: Component<Props, Result>,
  props: Props | null,
  key?: unknown,
): Result;
export declare function jsx(
  type: typeof Fragment,
  props: Record<string, unknown> | null,
  key?: unknown,
): unknown;
export declare const jsxs: typeof jsx;
export { Fragment };

export {};
