# Config

`hypreact` is configured from a config root directory that contains a `config.ts`, `config.tsx`, `config.js`, or `config.jsx` file.

In the Hyprland plugin, `config_path` should point to the config directory. The plugin looks for `config.ts`, `config.tsx`, `config.js`, or `config.jsx` inside it.

If `config_path` is omitted, the plugin falls back to `~/.config/hypreact` and probes those same entry names there.

If that config root does not exist yet, the plugin bootstraps it from the repo `template/`.

The current recommended shape is:

```ts
import type { HypreactConfig } from "@hypreact/sdk/config";

export default {
  defaultLayout: "master-stack",
  layoutRules: [
    { index: 0, layout: "master-stack" },
    { index: 4, layout: "testing" },
    { index: 5, layout: "primary-stack" },
    { monitor: "eDP-1", layout: "master-stack" },
  ],
  resize: {
    stepPx: 96,
    minBranchSizePx: 120,
  },
} satisfies HypreactConfig;
```

## Fields

### `defaultLayout`

- fallback layout name when no rule matches

### `layoutRules`

Ordered rules that select layouts for workspaces.

Supported rule fields:

- `layout: string` required
- `index?: number`
- `name?: string`
- `monitor?: string`

Notes:

- workspace `index` is numeric and zero-based
- workspace name `"1"` maps to index `0`
- workspace name `"5"` maps to index `4`
- more specific rules win over broader ones
- among equally specific rules, later rules win

Examples:

```ts
layoutRules: [
  { index: 0, layout: "master-stack" },
  { index: 1, layout: "primary-stack" },
  { name: "special", layout: "testing" },
  { monitor: "eDP-1", layout: "master-stack" },
  { index: 4, monitor: "eDP-1", layout: "testing" },
]
```

### `resize`

Top-level runtime policy knobs for resize behavior.

#### `resize.stepPx`

- requested pixel delta per resize command
- translated into internal share-space per active partition

#### `resize.minBranchSizePx`

- practical minimum inferred branch size on the partition main axis
- used to derive minimum share constraints for flex-inferred partitions

## Layout Discovery

`hypreact` discovers layouts from layout entry files such as:

- `layouts/master-stack/index.tsx`
- `layouts/primary-stack/index.tsx`

Associated CSS is read from the sibling `index.css` in the same layout directory.

## Recommended Project Layout

```text
config.ts
index.css
components/
layouts/
  master-stack/
    index.tsx
    index.css
.sdk/
  tsconfig.json
  config.d.ts
  layout.d.ts
  jsx-runtime.js
```

## SDK Support

The runtime JS modules for `@hypreact/sdk/*` are virtual at runtime, but the plugin also syncs editor-facing SDK files into the config root for authoring support.

Managed paths under the config root:

- `.sdk/`
- `.sdk/tsconfig.json`
- `.sdk/package.json`
- `.sdk/src/*.d.ts`
- `.sdk/src/*.js`

The recommended `tsconfig.json` in the config root should extend `./.sdk/tsconfig.json`.

That keeps external config directories self-contained for TypeScript and editor tooling without depending on this repo checkout.
