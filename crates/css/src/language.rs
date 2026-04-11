#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupportStatus {
    Full,
    Partial,
    Planned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SelectorTarget {
    Workspace,
    Group,
    Window,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StyleTarget {
    Workspace,
    Group,
    Window,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PropertySpec {
    pub name: &'static str,
    pub status: SupportStatus,
    pub applies_to: &'static [StyleTarget],
    pub value_keywords: &'static [&'static str],
    pub hover: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttributeKeySpec {
    pub name: &'static str,
    pub targets: &'static [SelectorTarget],
    pub hover: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PseudoClassSpec {
    pub name: &'static str,
    pub hover: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PseudoElementSpec {
    pub name: &'static str,
    pub targets: &'static [SelectorTarget],
    pub hover: &'static str,
}

pub const SELECTOR_TARGETS: &[SelectorTarget] =
    &[SelectorTarget::Workspace, SelectorTarget::Group, SelectorTarget::Window];

pub const INVALID_SELECTOR_TARGET_NAMES: &[&str] = &["slot"];

const ALL_STYLE_TARGETS: &[StyleTarget] =
    &[StyleTarget::Workspace, StyleTarget::Group, StyleTarget::Window];

const ALL_ELEMENT_TARGETS: &[StyleTarget] =
    &[StyleTarget::Workspace, StyleTarget::Group, StyleTarget::Window];

const WINDOW_ONLY_TARGETS: &[StyleTarget] = &[StyleTarget::Window];

const NO_KEYWORDS: &[&str] = &[];
const DISPLAY_KEYWORDS: &[&str] = &["flex", "grid", "none"];
const BOX_SIZING_KEYWORDS: &[&str] = &["border-box", "content-box"];
const APPEARANCE_KEYWORDS: &[&str] = &["auto", "none"];
const TEXT_ALIGN_KEYWORDS: &[&str] = &["left", "center", "right"];
const TEXT_TRANSFORM_KEYWORDS: &[&str] = &["none", "uppercase", "lowercase", "capitalize"];
const ANIMATION_DIRECTION_KEYWORDS: &[&str] =
    &["normal", "reverse", "alternate", "alternate-reverse"];
const ANIMATION_FILL_MODE_KEYWORDS: &[&str] = &["none", "forwards", "backwards", "both"];
const ANIMATION_PLAY_STATE_KEYWORDS: &[&str] = &["running", "paused"];
const TRANSITION_BEHAVIOR_KEYWORDS: &[&str] = &["normal", "allow-discrete"];
const FLEX_DIRECTION_KEYWORDS: &[&str] = &["row", "row-reverse", "column", "column-reverse"];
const FLEX_WRAP_KEYWORDS: &[&str] = &["nowrap", "wrap", "wrap-reverse"];
const POSITION_KEYWORDS: &[&str] = &["relative", "absolute"];
const OVERFLOW_KEYWORDS: &[&str] = &["visible", "hidden", "scroll", "clip"];
const ALIGN_ITEMS_KEYWORDS: &[&str] =
    &["stretch", "start", "end", "center", "flex-start", "flex-end"];
const ALIGN_SELF_KEYWORDS: &[&str] =
    &["auto", "stretch", "start", "end", "center", "flex-start", "flex-end"];
const JUSTIFY_ITEMS_KEYWORDS: &[&str] = &["stretch", "start", "end", "center"];
const JUSTIFY_SELF_KEYWORDS: &[&str] = &["auto", "stretch", "start", "end", "center"];
const ALIGN_CONTENT_KEYWORDS: &[&str] = &[
    "stretch",
    "start",
    "end",
    "center",
    "space-between",
    "space-around",
    "space-evenly",
    "flex-start",
    "flex-end",
];
const JUSTIFY_CONTENT_KEYWORDS: &[&str] = &[
    "stretch",
    "start",
    "end",
    "center",
    "space-between",
    "space-around",
    "space-evenly",
    "flex-start",
    "flex-end",
];
const GRID_AUTO_FLOW_KEYWORDS: &[&str] = &["row", "column", "dense", "row dense", "column dense"];

pub const ATTRIBUTE_KEY_SPECS: &[AttributeKeySpec] = &[
    AttributeKeySpec {
        name: "app_id",
        targets: &[SelectorTarget::Window],
        hover: "Matches a window's app_id metadata exactly.",
    },
    AttributeKeySpec {
        name: "title",
        targets: &[SelectorTarget::Window],
        hover: "Matches a window's title metadata exactly.",
    },
    AttributeKeySpec {
        name: "class",
        targets: &[SelectorTarget::Window],
        hover: "Matches a window's class metadata exactly.",
    },
    AttributeKeySpec {
        name: "instance",
        targets: &[SelectorTarget::Window],
        hover: "Matches a window's instance metadata exactly.",
    },
    AttributeKeySpec {
        name: "role",
        targets: &[SelectorTarget::Window],
        hover: "Matches a window's role metadata exactly.",
    },
    AttributeKeySpec {
        name: "shell",
        targets: &[SelectorTarget::Window],
        hover: "Matches a window's shell metadata exactly.",
    },
    AttributeKeySpec {
        name: "window_type",
        targets: &[SelectorTarget::Window],
        hover: "Matches a window's window_type metadata exactly.",
    },
];

pub const PSEUDO_CLASS_SPECS: &[PseudoClassSpec] = &[
    PseudoClassSpec {
        name: "focused",
        hover: "Matches the currently focused runtime window state.",
    },
    PseudoClassSpec { name: "floating", hover: "Matches floating windows." },
    PseudoClassSpec { name: "fullscreen", hover: "Matches fullscreen windows." },
    PseudoClassSpec { name: "urgent", hover: "Matches urgent windows." },
    PseudoClassSpec {
        name: "closing",
        hover: "Matches windows that are in the wm-managed closing phase.",
    },
    PseudoClassSpec {
        name: "enter-from-left",
        hover: "Matches workspaces entering from the left during workspace transitions.",
    },
    PseudoClassSpec {
        name: "enter-from-right",
        hover: "Matches workspaces entering from the right during workspace transitions.",
    },
    PseudoClassSpec {
        name: "exit-to-left",
        hover: "Matches workspaces exiting to the left during workspace transitions.",
    },
    PseudoClassSpec {
        name: "exit-to-right",
        hover: "Matches workspaces exiting to the right during workspace transitions.",
    },
];

pub const PSEUDO_ELEMENT_SPECS: &[PseudoElementSpec] = &[];

pub const PROPERTY_SPECS: &[PropertySpec] = &[
    PropertySpec {
        name: "display",
        status: SupportStatus::Full,
        applies_to: ALL_ELEMENT_TARGETS,
        value_keywords: DISPLAY_KEYWORDS,
        hover: "Controls element participation in layout.",
    },
    PropertySpec {
        name: "box-sizing",
        status: SupportStatus::Full,
        applies_to: ALL_ELEMENT_TARGETS,
        value_keywords: BOX_SIZING_KEYWORDS,
        hover: "Controls whether declared sizes include border and padding.",
    },
    PropertySpec {
        name: "aspect-ratio",
        status: SupportStatus::Full,
        applies_to: ALL_ELEMENT_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Sets a preferred width-to-height ratio for layout sizing.",
    },
    PropertySpec {
        name: "appearance",
        status: SupportStatus::Partial,
        applies_to: WINDOW_ONLY_TARGETS,
        value_keywords: APPEARANCE_KEYWORDS,
        hover: "Controls window decoration behavior for windows.",
    },
    PropertySpec {
        name: "background",
        status: SupportStatus::Partial,
        applies_to: WINDOW_ONLY_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Sets a solid background color where compositor rendering consumes it.",
    },
    PropertySpec {
        name: "background-color",
        status: SupportStatus::Partial,
        applies_to: WINDOW_ONLY_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Sets a solid background color where compositor rendering consumes it.",
    },
    PropertySpec {
        name: "color",
        status: SupportStatus::Partial,
        applies_to: WINDOW_ONLY_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Sets compositor-rendered text color where supported.",
    },
    PropertySpec {
        name: "opacity",
        status: SupportStatus::Partial,
        applies_to: WINDOW_ONLY_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Controls compositor-managed opacity effects where currently implemented.",
    },
    PropertySpec {
        name: "border-color",
        status: SupportStatus::Partial,
        applies_to: WINDOW_ONLY_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Sets compositor-consumed border color where supported.",
    },
    PropertySpec {
        name: "border-style",
        status: SupportStatus::Partial,
        applies_to: WINDOW_ONLY_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Controls compositor border edge visibility where supported.",
    },
    PropertySpec {
        name: "border-radius",
        status: SupportStatus::Partial,
        applies_to: WINDOW_ONLY_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Controls rounded window corners where compositor rendering consumes it.",
    },
    PropertySpec {
        name: "box-shadow",
        status: SupportStatus::Partial,
        applies_to: WINDOW_ONLY_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Controls compositor shadow rendering where supported.",
    },
    PropertySpec {
        name: "backdrop-filter",
        status: SupportStatus::Planned,
        applies_to: WINDOW_ONLY_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Reserved for future compositor backdrop effects.",
    },
    PropertySpec {
        name: "transform",
        status: SupportStatus::Partial,
        applies_to: ALL_STYLE_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Applies typed transform operations, with runtime support currently partial.",
    },
    PropertySpec {
        name: "text-align",
        status: SupportStatus::Partial,
        applies_to: ALL_STYLE_TARGETS,
        value_keywords: TEXT_ALIGN_KEYWORDS,
        hover: "Controls text alignment where rendering consumes it.",
    },
    PropertySpec {
        name: "text-transform",
        status: SupportStatus::Partial,
        applies_to: ALL_STYLE_TARGETS,
        value_keywords: TEXT_TRANSFORM_KEYWORDS,
        hover: "Controls text casing where rendering consumes it.",
    },
    PropertySpec {
        name: "font-family",
        status: SupportStatus::Partial,
        applies_to: ALL_STYLE_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Controls font family selection where rendering consumes it.",
    },
    PropertySpec {
        name: "font-size",
        status: SupportStatus::Partial,
        applies_to: ALL_STYLE_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Controls font size where rendering consumes it.",
    },
    PropertySpec {
        name: "font-weight",
        status: SupportStatus::Partial,
        applies_to: ALL_STYLE_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Controls font weight where rendering consumes it.",
    },
    PropertySpec {
        name: "letter-spacing",
        status: SupportStatus::Partial,
        applies_to: ALL_STYLE_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Controls letter spacing where rendering consumes it.",
    },
    PropertySpec {
        name: "animation",
        status: SupportStatus::Full,
        applies_to: ALL_STYLE_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Animation shorthand that expands to the supported longhands.",
    },
    PropertySpec {
        name: "animation-name",
        status: SupportStatus::Full,
        applies_to: ALL_STYLE_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "References one or more `@keyframes` blocks by name.",
    },
    PropertySpec {
        name: "animation-duration",
        status: SupportStatus::Full,
        applies_to: ALL_STYLE_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Sets animation durations.",
    },
    PropertySpec {
        name: "animation-timing-function",
        status: SupportStatus::Full,
        applies_to: ALL_STYLE_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Sets animation easing functions.",
    },
    PropertySpec {
        name: "animation-delay",
        status: SupportStatus::Full,
        applies_to: ALL_STYLE_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Sets animation delays.",
    },
    PropertySpec {
        name: "animation-iteration-count",
        status: SupportStatus::Full,
        applies_to: ALL_STYLE_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Sets how many times an animation repeats.",
    },
    PropertySpec {
        name: "animation-direction",
        status: SupportStatus::Full,
        applies_to: ALL_STYLE_TARGETS,
        value_keywords: ANIMATION_DIRECTION_KEYWORDS,
        hover: "Sets animation playback direction.",
    },
    PropertySpec {
        name: "animation-fill-mode",
        status: SupportStatus::Full,
        applies_to: ALL_STYLE_TARGETS,
        value_keywords: ANIMATION_FILL_MODE_KEYWORDS,
        hover: "Sets animation fill behavior before and after playback.",
    },
    PropertySpec {
        name: "animation-play-state",
        status: SupportStatus::Full,
        applies_to: ALL_STYLE_TARGETS,
        value_keywords: ANIMATION_PLAY_STATE_KEYWORDS,
        hover: "Sets whether animations are running or paused.",
    },
    PropertySpec {
        name: "transition",
        status: SupportStatus::Full,
        applies_to: ALL_STYLE_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Transition shorthand that expands to the supported longhands.",
    },
    PropertySpec {
        name: "transition-property",
        status: SupportStatus::Full,
        applies_to: ALL_STYLE_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Selects which properties transition.",
    },
    PropertySpec {
        name: "transition-duration",
        status: SupportStatus::Full,
        applies_to: ALL_STYLE_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Sets transition durations.",
    },
    PropertySpec {
        name: "transition-timing-function",
        status: SupportStatus::Full,
        applies_to: ALL_STYLE_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Sets transition easing functions.",
    },
    PropertySpec {
        name: "transition-delay",
        status: SupportStatus::Full,
        applies_to: ALL_STYLE_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Sets transition delays.",
    },
    PropertySpec {
        name: "transition-behavior",
        status: SupportStatus::Partial,
        applies_to: ALL_STYLE_TARGETS,
        value_keywords: TRANSITION_BEHAVIOR_KEYWORDS,
        hover: "Accepted for compatibility but currently ignored by runtime compilation.",
    },
    PropertySpec {
        name: "flex-direction",
        status: SupportStatus::Full,
        applies_to: ALL_ELEMENT_TARGETS,
        value_keywords: FLEX_DIRECTION_KEYWORDS,
        hover: "Controls the main axis direction for flex layout.",
    },
    PropertySpec {
        name: "flex-wrap",
        status: SupportStatus::Full,
        applies_to: ALL_ELEMENT_TARGETS,
        value_keywords: FLEX_WRAP_KEYWORDS,
        hover: "Controls whether flex items wrap.",
    },
    PropertySpec {
        name: "flex-grow",
        status: SupportStatus::Full,
        applies_to: ALL_ELEMENT_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Controls flex growth contribution.",
    },
    PropertySpec {
        name: "flex-shrink",
        status: SupportStatus::Full,
        applies_to: ALL_ELEMENT_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Controls flex shrink contribution.",
    },
    PropertySpec {
        name: "flex-basis",
        status: SupportStatus::Full,
        applies_to: ALL_ELEMENT_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Sets the initial main-size basis for flex items.",
    },
    PropertySpec {
        name: "position",
        status: SupportStatus::Full,
        applies_to: ALL_ELEMENT_TARGETS,
        value_keywords: POSITION_KEYWORDS,
        hover: "Controls normal or absolute positioning.",
    },
    PropertySpec {
        name: "inset",
        status: SupportStatus::Full,
        applies_to: ALL_ELEMENT_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Shorthand for absolute positioning offsets.",
    },
    PropertySpec {
        name: "top",
        status: SupportStatus::Full,
        applies_to: ALL_ELEMENT_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Sets the top offset for positioned elements.",
    },
    PropertySpec {
        name: "right",
        status: SupportStatus::Full,
        applies_to: ALL_ELEMENT_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Sets the right offset for positioned elements.",
    },
    PropertySpec {
        name: "bottom",
        status: SupportStatus::Full,
        applies_to: ALL_ELEMENT_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Sets the bottom offset for positioned elements.",
    },
    PropertySpec {
        name: "left",
        status: SupportStatus::Full,
        applies_to: ALL_ELEMENT_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Sets the left offset for positioned elements.",
    },
    PropertySpec {
        name: "overflow",
        status: SupportStatus::Full,
        applies_to: ALL_ELEMENT_TARGETS,
        value_keywords: OVERFLOW_KEYWORDS,
        hover: "Controls overflow behavior on both axes.",
    },
    PropertySpec {
        name: "overflow-x",
        status: SupportStatus::Full,
        applies_to: ALL_ELEMENT_TARGETS,
        value_keywords: OVERFLOW_KEYWORDS,
        hover: "Controls horizontal overflow behavior.",
    },
    PropertySpec {
        name: "overflow-y",
        status: SupportStatus::Full,
        applies_to: ALL_ELEMENT_TARGETS,
        value_keywords: OVERFLOW_KEYWORDS,
        hover: "Controls vertical overflow behavior.",
    },
    PropertySpec {
        name: "width",
        status: SupportStatus::Full,
        applies_to: ALL_STYLE_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Sets preferred width.",
    },
    PropertySpec {
        name: "height",
        status: SupportStatus::Full,
        applies_to: ALL_STYLE_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Sets preferred height.",
    },
    PropertySpec {
        name: "min-width",
        status: SupportStatus::Full,
        applies_to: ALL_ELEMENT_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Sets the minimum width.",
    },
    PropertySpec {
        name: "min-height",
        status: SupportStatus::Full,
        applies_to: ALL_ELEMENT_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Sets the minimum height.",
    },
    PropertySpec {
        name: "max-width",
        status: SupportStatus::Full,
        applies_to: ALL_ELEMENT_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Sets the maximum width.",
    },
    PropertySpec {
        name: "max-height",
        status: SupportStatus::Full,
        applies_to: ALL_ELEMENT_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Sets the maximum height.",
    },
    PropertySpec {
        name: "align-items",
        status: SupportStatus::Full,
        applies_to: ALL_ELEMENT_TARGETS,
        value_keywords: ALIGN_ITEMS_KEYWORDS,
        hover: "Controls item alignment within the container.",
    },
    PropertySpec {
        name: "align-self",
        status: SupportStatus::Full,
        applies_to: ALL_ELEMENT_TARGETS,
        value_keywords: ALIGN_SELF_KEYWORDS,
        hover: "Overrides alignment for an individual item.",
    },
    PropertySpec {
        name: "justify-items",
        status: SupportStatus::Full,
        applies_to: ALL_ELEMENT_TARGETS,
        value_keywords: JUSTIFY_ITEMS_KEYWORDS,
        hover: "Controls inline-axis item alignment.",
    },
    PropertySpec {
        name: "justify-self",
        status: SupportStatus::Full,
        applies_to: ALL_ELEMENT_TARGETS,
        value_keywords: JUSTIFY_SELF_KEYWORDS,
        hover: "Overrides inline-axis alignment for an individual item.",
    },
    PropertySpec {
        name: "align-content",
        status: SupportStatus::Full,
        applies_to: ALL_ELEMENT_TARGETS,
        value_keywords: ALIGN_CONTENT_KEYWORDS,
        hover: "Controls distribution of lines or tracks on the block axis.",
    },
    PropertySpec {
        name: "justify-content",
        status: SupportStatus::Full,
        applies_to: ALL_ELEMENT_TARGETS,
        value_keywords: JUSTIFY_CONTENT_KEYWORDS,
        hover: "Controls distribution of lines or tracks on the inline axis.",
    },
    PropertySpec {
        name: "gap",
        status: SupportStatus::Full,
        applies_to: ALL_ELEMENT_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Sets row and column gaps between children.",
    },
    PropertySpec {
        name: "row-gap",
        status: SupportStatus::Full,
        applies_to: ALL_ELEMENT_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Sets vertical gaps between children.",
    },
    PropertySpec {
        name: "column-gap",
        status: SupportStatus::Full,
        applies_to: ALL_ELEMENT_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Sets horizontal gaps between children.",
    },
    PropertySpec {
        name: "grid-template-rows",
        status: SupportStatus::Full,
        applies_to: ALL_ELEMENT_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Defines explicit grid row tracks.",
    },
    PropertySpec {
        name: "grid-template-columns",
        status: SupportStatus::Full,
        applies_to: ALL_ELEMENT_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Defines explicit grid column tracks.",
    },
    PropertySpec {
        name: "grid-auto-rows",
        status: SupportStatus::Full,
        applies_to: ALL_ELEMENT_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Defines implicit grid row sizing.",
    },
    PropertySpec {
        name: "grid-auto-columns",
        status: SupportStatus::Full,
        applies_to: ALL_ELEMENT_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Defines implicit grid column sizing.",
    },
    PropertySpec {
        name: "grid-auto-flow",
        status: SupportStatus::Full,
        applies_to: ALL_ELEMENT_TARGETS,
        value_keywords: GRID_AUTO_FLOW_KEYWORDS,
        hover: "Controls grid auto-placement direction.",
    },
    PropertySpec {
        name: "grid-template-areas",
        status: SupportStatus::Full,
        applies_to: ALL_ELEMENT_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Defines named grid areas.",
    },
    PropertySpec {
        name: "grid-row",
        status: SupportStatus::Full,
        applies_to: ALL_ELEMENT_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Places an item across grid row lines.",
    },
    PropertySpec {
        name: "grid-column",
        status: SupportStatus::Full,
        applies_to: ALL_ELEMENT_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Places an item across grid column lines.",
    },
    PropertySpec {
        name: "grid-row-start",
        status: SupportStatus::Full,
        applies_to: ALL_ELEMENT_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Sets the starting grid row line.",
    },
    PropertySpec {
        name: "grid-row-end",
        status: SupportStatus::Full,
        applies_to: ALL_ELEMENT_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Sets the ending grid row line.",
    },
    PropertySpec {
        name: "grid-column-start",
        status: SupportStatus::Full,
        applies_to: ALL_ELEMENT_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Sets the starting grid column line.",
    },
    PropertySpec {
        name: "grid-column-end",
        status: SupportStatus::Full,
        applies_to: ALL_ELEMENT_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Sets the ending grid column line.",
    },
    PropertySpec {
        name: "border-width",
        status: SupportStatus::Partial,
        applies_to: WINDOW_ONLY_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Sets compositor-consumed border widths where supported.",
    },
    PropertySpec {
        name: "border-top-width",
        status: SupportStatus::Partial,
        applies_to: WINDOW_ONLY_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Sets the top border width where compositor rendering consumes it.",
    },
    PropertySpec {
        name: "border-right-width",
        status: SupportStatus::Partial,
        applies_to: WINDOW_ONLY_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Sets the right border width where compositor rendering consumes it.",
    },
    PropertySpec {
        name: "border-bottom-width",
        status: SupportStatus::Partial,
        applies_to: WINDOW_ONLY_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Sets the bottom border width where compositor rendering consumes it.",
    },
    PropertySpec {
        name: "border-left-width",
        status: SupportStatus::Partial,
        applies_to: WINDOW_ONLY_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Sets the left border width where compositor rendering consumes it.",
    },
    PropertySpec {
        name: "border-top-color",
        status: SupportStatus::Partial,
        applies_to: WINDOW_ONLY_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Sets the top border color where compositor rendering consumes it.",
    },
    PropertySpec {
        name: "border-right-color",
        status: SupportStatus::Partial,
        applies_to: WINDOW_ONLY_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Sets the right border color where compositor rendering consumes it.",
    },
    PropertySpec {
        name: "border-bottom-color",
        status: SupportStatus::Partial,
        applies_to: WINDOW_ONLY_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Sets the bottom border color where compositor rendering consumes it.",
    },
    PropertySpec {
        name: "border-left-color",
        status: SupportStatus::Partial,
        applies_to: WINDOW_ONLY_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Sets the left border color where compositor rendering consumes it.",
    },
    PropertySpec {
        name: "border-top-style",
        status: SupportStatus::Partial,
        applies_to: WINDOW_ONLY_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Sets the top border style where compositor rendering consumes it.",
    },
    PropertySpec {
        name: "border-right-style",
        status: SupportStatus::Partial,
        applies_to: WINDOW_ONLY_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Sets the right border style where compositor rendering consumes it.",
    },
    PropertySpec {
        name: "border-bottom-style",
        status: SupportStatus::Partial,
        applies_to: WINDOW_ONLY_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Sets the bottom border style where compositor rendering consumes it.",
    },
    PropertySpec {
        name: "border-left-style",
        status: SupportStatus::Partial,
        applies_to: WINDOW_ONLY_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Sets the left border style where compositor rendering consumes it.",
    },
    PropertySpec {
        name: "padding",
        status: SupportStatus::Full,
        applies_to: ALL_STYLE_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Sets padding on all four sides.",
    },
    PropertySpec {
        name: "padding-top",
        status: SupportStatus::Full,
        applies_to: ALL_STYLE_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Sets top padding.",
    },
    PropertySpec {
        name: "padding-right",
        status: SupportStatus::Full,
        applies_to: ALL_STYLE_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Sets right padding.",
    },
    PropertySpec {
        name: "padding-bottom",
        status: SupportStatus::Full,
        applies_to: ALL_STYLE_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Sets bottom padding.",
    },
    PropertySpec {
        name: "padding-left",
        status: SupportStatus::Full,
        applies_to: ALL_STYLE_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Sets left padding.",
    },
    PropertySpec {
        name: "margin",
        status: SupportStatus::Full,
        applies_to: ALL_ELEMENT_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Sets margins on all four sides.",
    },
    PropertySpec {
        name: "margin-top",
        status: SupportStatus::Full,
        applies_to: ALL_ELEMENT_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Sets top margin.",
    },
    PropertySpec {
        name: "margin-right",
        status: SupportStatus::Full,
        applies_to: ALL_ELEMENT_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Sets right margin.",
    },
    PropertySpec {
        name: "margin-bottom",
        status: SupportStatus::Full,
        applies_to: ALL_ELEMENT_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Sets bottom margin.",
    },
    PropertySpec {
        name: "margin-left",
        status: SupportStatus::Full,
        applies_to: ALL_ELEMENT_TARGETS,
        value_keywords: NO_KEYWORDS,
        hover: "Sets left margin.",
    },
];

pub fn property_specs() -> &'static [PropertySpec] {
    PROPERTY_SPECS
}

pub fn property_spec(name: &str) -> Option<&'static PropertySpec> {
    PROPERTY_SPECS.iter().find(|spec| spec.name == name)
}

pub fn is_supported_property(name: &str) -> bool {
    property_spec(name).is_some()
}

pub fn attribute_key_specs() -> &'static [AttributeKeySpec] {
    ATTRIBUTE_KEY_SPECS
}

pub fn attribute_key_spec(name: &str) -> Option<&'static AttributeKeySpec> {
    ATTRIBUTE_KEY_SPECS.iter().find(|spec| spec.name == name)
}

pub fn is_supported_attribute_key(name: &str) -> bool {
    attribute_key_spec(name).is_some()
}

pub fn pseudo_class_specs() -> &'static [PseudoClassSpec] {
    PSEUDO_CLASS_SPECS
}

pub fn pseudo_class_spec(name: &str) -> Option<&'static PseudoClassSpec> {
    PSEUDO_CLASS_SPECS.iter().find(|spec| spec.name == name)
}

pub fn is_supported_pseudo_class(name: &str) -> bool {
    pseudo_class_spec(name).is_some()
}

pub fn pseudo_element_specs() -> &'static [PseudoElementSpec] {
    PSEUDO_ELEMENT_SPECS
}

pub fn pseudo_element_spec(name: &str) -> Option<&'static PseudoElementSpec> {
    PSEUDO_ELEMENT_SPECS.iter().find(|spec| spec.name == name)
}

pub fn is_supported_pseudo_element(name: &str) -> bool {
    pseudo_element_spec(name).is_some()
}

pub fn is_invalid_selector_target(name: &str) -> bool {
    INVALID_SELECTOR_TARGET_NAMES.contains(&name)
}
