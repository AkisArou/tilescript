import "@tilescript/sdk/css.d.ts";
import type { TilescriptConfig } from "@tilescript/sdk/config";

export default {
  defaultLayout: "master-stack",
  layoutRules: [{ index: 0, layout: "master-stack" }],
  resize: {
    stepPx: 96,
    minBranchSizePx: 120,
  },
} satisfies TilescriptConfig;
