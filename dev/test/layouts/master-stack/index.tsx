import type { LayoutContext } from "@tilescript/sdk/layout";

import "./index.css";

export default function layout(ctx: LayoutContext) {
  const hasAlacritty = ctx.windows.some(
    (window) => window.class === "Alacritty",
  );

  const mainWindowCount = ctx.windows.length - Number(hasAlacritty);

  return (
    <workspace>
      <group moveAs="group">
        <window
          id="alacritty-column"
          match='class="Alacritty"'
          class="alacritty-column"
        />
      </group>

      <group class="main-area" moveAs="group">
        <slot take={1} class="master-slot" />

        {mainWindowCount > 1 ? (
          <group class="stack-group">
            <slot class="stack-slot" />
          </group>
        ) : null}
      </group>
    </workspace>
  );
}
