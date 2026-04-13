export type LayoutRule =
  | { layout: string; index: number; monitor?: string; name?: never }
  | { layout: string; name: string; index?: never; monitor?: string }
  | { layout: string; monitor: string; index?: never; name?: never }
  | { layout: string; index?: never; name?: never; monitor?: never };

export interface HypreactConfig {
  defaultLayout?: string;
  layoutRules?: LayoutRule[];
  resize?: {
    stepPx?: number;
    minBranchSizePx?: number;
  };
}
