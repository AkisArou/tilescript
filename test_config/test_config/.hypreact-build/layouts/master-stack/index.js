import { sp } from "@hypreact/sdk/jsx-runtime";

export default function layout(ctx) {
	return /* @__PURE__ */ sp("workspace", { id: "frame" }, /* @__PURE__ */ sp("slot", {
		id: "master",
		take: 1,
		class: "master-slot"
	}), ctx.windows.length > 1 ? /* @__PURE__ */ sp("group", { class: "stack-group" }, /* @__PURE__ */ sp("slot", {
		id: "stack-slot",
		class: "stack-group__item"
	})) : null);
}
