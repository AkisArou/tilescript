import { sp } from "@hypreact/sdk/jsx-runtime";

export default function layout(ctx) {
	return /* @__PURE__ */ sp("workspace", { id: "root" }, /* @__PURE__ */ sp("group", { id: "frame" }, /* @__PURE__ */ sp("slot", {
		id: "master",
		take: 1
	}), ctx.windows.length > 1 && /* @__PURE__ */ sp("group", { id: "stack" }, /* @__PURE__ */ sp("slot", { class: "stack-item" }))));
}
