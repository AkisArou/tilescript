import { Fragment, jsx } from "./jsx-runtime.js";

export { Fragment };

export function jsxDEV(type, props, key) {
  return jsx(type, props, key);
}
