/** @jsxImportSource @tilescript/sdk */

import type { SlotProps } from "@tilescript/sdk/layout";

export function StackSlot(props: SlotProps) {
  return <slot class="stack-slot" {...props} />;
}
