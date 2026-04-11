import { Fragment, type JSX } from "./jsx-runtime.js";

export { Fragment };
export type { JSX };

type Component<Props = Record<string, unknown>> = (props: Props) => unknown;

export declare function jsxDEV(
  type: string | typeof Fragment | Component,
  props: Record<string, unknown> | null,
  key?: unknown,
): unknown;
