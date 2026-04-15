---@meta

---@class Hypreact.LayoutWindow
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

---@class Hypreact.LayoutContextMonitor
---@field name string
---@field width integer
---@field height integer
---@field scale number|nil

---@class Hypreact.LayoutContextWorkspace
---@field name string
---@field workspaces string[]|nil
---@field windowCount integer

---@class Hypreact.LayoutContext
---@field monitor Hypreact.LayoutContextMonitor
---@field workspace Hypreact.LayoutContextWorkspace
---@field windows Hypreact.LayoutWindow[]
---@field state table<string, unknown>|nil

---@class Hypreact.LayoutBaseProps
---@field id string|nil
---@field class string|nil

---@class Hypreact.WorkspaceProps: Hypreact.LayoutBaseProps
---@class Hypreact.GroupProps: Hypreact.LayoutBaseProps

---@class Hypreact.SlotProps: Hypreact.LayoutBaseProps
---@field take integer|nil

---@class Hypreact.WindowProps: Hypreact.LayoutBaseProps
---@field match string|nil

---@class Hypreact.LayoutNode
---@field type "workspace"|"group"|"slot"|"window"
---@field props table<string, unknown>
---@field children Hypreact.LayoutNode[]|nil

---@alias Hypreact.Child Hypreact.LayoutNode|Hypreact.LayoutNode[]|nil

---@alias Hypreact.ContainerBuilder fun(children: Hypreact.Child[]): Hypreact.LayoutNode
---@alias Hypreact.ConditionalBuilder fun(children: Hypreact.Child[]): Hypreact.Child

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
---@return Hypreact.ContainerBuilder
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
---@return Hypreact.LayoutNode
local function leaf(type_name, props)
  return {
    type = type_name,
    props = props or {},
  }
end

---@class Hypreact.Module
local M = {}

---@param props Hypreact.WorkspaceProps|nil
---@return Hypreact.ContainerBuilder
function M.workspace(props)
  return container("workspace", props)
end

---@param props Hypreact.GroupProps|nil
---@return Hypreact.ContainerBuilder
function M.group(props)
  return container("group", props)
end

---@param props Hypreact.SlotProps|nil
---@return Hypreact.LayoutNode
function M.slot(props)
  return leaf("slot", props)
end

---@param props Hypreact.WindowProps|nil
---@return Hypreact.LayoutNode
function M.window(props)
  return leaf("window", props)
end

---@param condition boolean
---@return Hypreact.ConditionalBuilder
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
