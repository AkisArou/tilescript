local h = require("hypreact")

---@param ctx Hypreact.LayoutContext
return function(ctx)
  return h.workspace({ id = "frame" }) {
    h.slot({
      id = "master",
      take = 1,
      class = "master-slot",
    }),

    h.when(#ctx.windows > 1) {
      h.group({ class = "stack-group" }) {
        h.slot({
          id = "stack-slot",
          class = "stack-group__item",
        }),
      },
    },
  }
end
