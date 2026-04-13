import type { HypreactConfig } from "@hypreact/sdk/config";

import { layouts } from "./config/layouts.ts";

export default {
  layouts,
  resize: {
    step_px: 96,
    min_branch_size_px: 120,
  },
} satisfies HypreactConfig;
