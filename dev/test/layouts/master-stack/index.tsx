import type { LayoutContext } from "@tilescript/sdk/layout";

import "./index.css";

export default function layout(ctx: LayoutContext) {
  const hasAlacritty = ctx.windows.some((window) => window.class === "Alacritty");
  const mainWindowCount = ctx.windows.length - Number(hasAlacritty);

  return (
    <workspace id="frame">
      <group moveAs="group">
        <window
          id="alacritty-column"
          match='class="Alacritty"'
          class="alacritty-column"
        />
      </group>

      <group id="main-area" moveAs="group">
        <slot id="master" take={1} class="master-slot" />

        {mainWindowCount > 1 ? (
          <group class="stack-group">
            <slot id="stack-slot" class="stack-group__item" />
          </group>
        ) : null}
      </group>
    </workspace>
  );
}
