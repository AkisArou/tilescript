/** @jsxImportSource @tilescript/sdk */

import type { LayoutContext } from "@tilescript/sdk/layout";
import "./index.css";

export default function layout(ctx: LayoutContext) {
  return (
    <workspace>
      <group class="frame">
        <slot take={1} class="master-slot" />
        {ctx.windows.length > 1 && (
          <group class="stack-column">
            <slot class="stack-item" />
          </group>
        )}
      </group>
    </workspace>
  );
}
