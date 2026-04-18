# JSX Layouts

Layout modules return structural trees from TypeScript or JavaScript. They describe arrangement semantics, not pixels.

The runtime evaluates layout JSX, Rust validates the tree structure, resolves window claims, applies layout CSS, computes final geometry, and communicates assignments to the compositor.

## Elements

Supported JSX elements:

- `workspace`
- `group`
- `window`
- `slot`

## Element Rules

### `workspace`

- must be the root element
- represents the current workspace area
- can contain `group`, `window`, and `slot`

### `group`

- pure structural container
- can contain `group`, `window`, and `slot`

### `window`

- claims at most one matching unclaimed window
- suited to explicit placements

### `slot`

- claims zero or more matching unclaimed windows
- suited to stacks, sidebars, and catch-all regions

## Props

Common props:

- `id?: string`
- `class?: string`
- `children?: LayoutChildren`

`window` props:

- `match?: string`

`slot` props:

- `take?: number`

## Matching

`match` uses exact string clauses joined by spaces.

Format:

```text
key="value" key="value"
```

Supported keys:

- `app_id`
- `title`
- `class`
- `instance`
- `role`
- `shell`
- `window_type`

All clauses are ANDed.

## `take`

`slot` supports:

- omitted: claim remaining matches
- positive integer: claim up to that many matches

## Layout Context

Layout functions receive:

```ts
interface LayoutContext {
  monitor: {
    name: string;
    width: number;
    height: number;
    scale?: number;
  };
  workspace: {
    name: string;
    workspaces?: string[];
    windowCount: number;
  };
  windows: LayoutWindow[];
  state?: Record<string, unknown>;
}
```

## Minimal Example

```tsx
export default function Layout() {
  return <workspace />;
}
```

## Columns Example

```tsx
export default function Layout() {
  return (
    <workspace>
      <group id="main" class="stack">
        <window match='app_id="foot"' />
        <slot />
      </group>
    </workspace>
  );
}
```

## Sidebar Example

```tsx
export default function Layout() {
  return (
    <workspace>
      <group id="content">
        <slot />
      </group>
      <group id="sidebar">
        <window match='app_id="slack"' />
        <window match='app_id="discord"' />
      </group>
    </workspace>
  );
}
```

## Behavior Notes

- unmatched `window` nodes are omitted from the resolved tree
- claim order is document order; later nodes only see windows not claimed by earlier nodes
- floating windows are compositor-rendered and can override tiled placement rules
- layout functions are called on every state change; return consistent results to avoid flickering
- slot `take` limits window captures to specific regions
