import { sp } from "@hypreact/sdk/jsx-runtime";

export default function layout() {
	return /* @__PURE__ */ sp("workspace", { id: "root" }, /* @__PURE__ */ sp("slot", { id: "main" }));
}
