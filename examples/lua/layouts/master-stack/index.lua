local h = require("tilescript")

---@param ctx Tilescript.LayoutContext
return function(ctx)
  return h.workspace() {
    h.slot({
      take = 1,
      class = "master-slot",
    }),

    h.when(#ctx.windows > 1) {
      h.group({ class = "stack-group" }) {
        h.slot({
          class = "stack-slot",
        }),
      },
    },
  }
end
