export interface LayoutsConfig {
  default?: string;
  per_workspace?: string[];
  per_monitor?: Record<string, string>;
}

export interface HypreactConfig {
  layouts?: LayoutsConfig;
  resize?: {
    step_px?: number;
    min_branch_size_px?: number;
  };
}
