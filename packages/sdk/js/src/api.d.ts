export interface HypreactRuntimeApi {
  /**
   * Placeholder authored-config surface only. Hypreact config evaluation does not
   * expose a live event or query bus inside the JS runtime.
   */
  readonly available: false;
}

export const runtime: HypreactRuntimeApi;
