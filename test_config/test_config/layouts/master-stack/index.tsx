import type { LayoutContext } from "@hypreact/sdk/layout";

import "./index.css";

export default function layout(ctx: LayoutContext) {
  return (
    <workspace id="frame" class="playground-workspace">
      <slot id="master" take={1} class="master-slot" />

      {ctx.windows.length > 1 ? (
        <group id="stack" class="stack-group">
          <slot id="stack-slot" class="stack-group__item" />
        </group>
      ) : null}
    </workspace>
  );
}
