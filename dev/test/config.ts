import "@tilescript/sdk/css.d.ts";

import type { TilescriptConfig } from "@tilescript/sdk/config";

export default {
  defaultLayout: "master-stack",
  layoutRules: [
    { index: 0, layout: "dwindle" },
    { index: 1, layout: "master-stack" },
    { index: 2, layout: "primary-stack" },
    { index: 3, layout: "master-stack" },
  ],
  attachAfterFocused: true,
  resize: {
    stepPx: 96,
    minBranchSizePx: 120,
  },
} satisfies TilescriptConfig;
