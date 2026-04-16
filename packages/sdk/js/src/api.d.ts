export interface TilescriptRuntimeApi {
  /**
   * Placeholder authored-config surface only. Tilescript config evaluation does not
   * expose a live event or query bus inside the JS runtime.
   */
  readonly available: false;
}

export const runtime: TilescriptRuntimeApi;
