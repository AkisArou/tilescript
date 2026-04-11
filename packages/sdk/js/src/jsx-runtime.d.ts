import type {
  GroupProps,
  LayoutRenderable,
  SlotProps,
  WindowProps,
  WorkspaceProps,
} from "./layout";

type Component<Props = Record<string, unknown>> = (props: Props) => unknown;
type JSXChild = LayoutRenderable | string | number | boolean | null | undefined;
type JSXChildren = JSXChild | JSXChild[];
type JSXPropsWithChildren<T> = T & {
  children?: JSXChildren;
};

declare global {
  const Fragment: unique symbol;
  function sp(
    type: string | typeof Fragment | Component,
    props: Record<string, unknown> | null,
    ...children: unknown[]
  ): unknown;

  namespace JSX {
    type Element = any;

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
      workspace: JSXPropsWithChildren<WorkspaceProps>;
      group: JSXPropsWithChildren<GroupProps>;
      slot: JSXPropsWithChildren<SlotProps>;
      window: JSXPropsWithChildren<WindowProps>;
    }

    type LibraryManagedAttributes<C, P> = JSXPropsWithChildren<P>;
  }
}

export declare function sp(
  type: string | typeof Fragment | Component,
  props: Record<string, unknown> | null,
  ...children: unknown[]
): unknown;
export declare function jsx(
  type: string | typeof Fragment | Component,
  props: Record<string, unknown> | null,
  key?: unknown,
): unknown;
export declare function jsxs(
  type: string | typeof Fragment | Component,
  props: Record<string, unknown> | null,
  key?: unknown,
): unknown;
export { Fragment };

export {};
