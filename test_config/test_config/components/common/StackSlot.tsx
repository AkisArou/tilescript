/** @jsxImportSource @hypreact/sdk */

import type { SlotProps } from "@hypreact/sdk/layout";

export function StackSlot(props: SlotProps) {
  return <slot class="stack-group__item" {...props} />;
}
