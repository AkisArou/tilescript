---@meta

---@class Tilescript.LayoutWindow
---@field id string
---@field app_id string|nil
---@field title string|nil
---@field class string|nil
---@field instance string|nil
---@field role string|nil
---@field shell string|nil
---@field window_type string|nil
---@field floating boolean|nil
---@field fullscreen boolean|nil
---@field focused boolean|nil

---@class Tilescript.LayoutContextMonitor
---@field name string
---@field width integer
---@field height integer
---@field scale number|nil

---@class Tilescript.LayoutContextWorkspace
---@field name string
---@field workspaces string[]|nil
---@field windowCount integer

---@class Tilescript.LayoutContext
---@field monitor Tilescript.LayoutContextMonitor
---@field workspace Tilescript.LayoutContextWorkspace
---@field windows Tilescript.LayoutWindow[]
---@field state table<string, unknown>|nil

---@class Tilescript.LayoutBaseProps
---@field id string|nil
---@field class string|nil

---@class Tilescript.WorkspaceProps: Tilescript.LayoutBaseProps
---@class Tilescript.GroupProps: Tilescript.LayoutBaseProps

---@class Tilescript.SlotProps: Tilescript.LayoutBaseProps
---@field take integer|nil

---@class Tilescript.WindowProps: Tilescript.LayoutBaseProps
---@field match string|nil

---@class Tilescript.LayoutNode
---@field type "workspace"|"group"|"slot"|"window"
---@field props table<string, unknown>
---@field children Tilescript.LayoutNode[]|nil

---@alias Tilescript.Child Tilescript.LayoutNode|Tilescript.LayoutNode[]|nil

---@alias Tilescript.ContainerBuilder fun(children: Tilescript.Child[]): Tilescript.LayoutNode
---@alias Tilescript.ConditionalBuilder fun(children: Tilescript.Child[]): Tilescript.Child

local function append_child(out, child)
  if child == nil then
    return
  end

  if type(child) == "table" and child.type == nil then
    for _, nested in ipairs(child) do
      append_child(out, nested)
    end
    return
  end

  out[#out + 1] = child
end

---@param type_name "workspace"|"group"
---@param props table<string, unknown>|nil
---@return Tilescript.ContainerBuilder
local function container(type_name, props)
  return function(children)
    local normalized = {}

    for _, child in ipairs(children or {}) do
      append_child(normalized, child)
    end

    return {
      type = type_name,
      props = props or {},
      children = normalized,
    }
  end
end

---@param type_name "slot"|"window"
---@param props table<string, unknown>|nil
---@return Tilescript.LayoutNode
local function leaf(type_name, props)
  return {
    type = type_name,
    props = props or {},
  }
end

---@class Tilescript.Module
local M = {}

---@param props Tilescript.WorkspaceProps|nil
---@return Tilescript.ContainerBuilder
function M.workspace(props)
  return container("workspace", props)
end

---@param props Tilescript.GroupProps|nil
---@return Tilescript.ContainerBuilder
function M.group(props)
  return container("group", props)
end

---@param props Tilescript.SlotProps|nil
---@return Tilescript.LayoutNode
function M.slot(props)
  return leaf("slot", props)
end

---@param props Tilescript.WindowProps|nil
---@return Tilescript.LayoutNode
function M.window(props)
  return leaf("window", props)
end

---@param condition boolean
---@return Tilescript.ConditionalBuilder
function M.when(condition)
  return function(children)
    if not condition then
      return nil
    end

    local normalized = {}
    for _, child in ipairs(children or {}) do
      append_child(normalized, child)
    end
    return normalized
  end
end

return M
