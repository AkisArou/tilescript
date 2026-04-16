/** @jsxImportSource @tilescript/sdk */

import "./StackGroup.css";
import type { GroupProps, LayoutContext } from "@tilescript/sdk/layout";

import { StackSlot } from "./common/StackSlot";

type StackGroupProps = GroupProps & {
  ctx: LayoutContext;
};

export function StackGroup({ ctx, children, ...props }: StackGroupProps) {
  if (ctx.windows.length <= 1) {
    return null;
  }

  return (
    <group id="stack" class="stack-group" {...props}>
      <StackSlot />
      {children}
    </group>
  );
}
