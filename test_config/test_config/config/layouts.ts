import type { LayoutsConfig } from "@hypreact/sdk/config";

export const layouts: LayoutsConfig = {
  default: "master-stack",
  per_workspace: [
    "master-stack",
    "master-stack",
    "master-stack",
    "master-stack",
    "testing",
    "primary-stack",
    "primary-stack",
    "primary-stack",
    "random",
  ],
  per_monitor: {
    "eDP-1": "master-stack",
  },
};
