export interface LayoutsConfig {
  default?: string;
  per_workspace?: string[];
  per_monitor?: Record<string, string>;
}

export interface HypreactConfig {
  layouts?: LayoutsConfig;
}
