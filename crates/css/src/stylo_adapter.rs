use std::fmt;

use cssparser::{ToCss, serialize_identifier};
use precomputed_hash::PrecomputedHash;
use selectors::attr::{AttrSelectorOperation, CaseSensitivity, NamespaceConstraint};
use selectors::context::{
    MatchingContext, MatchingForInvalidation, MatchingMode, NeedsSelectorFlags, QuirksMode,
    SelectorCaches,
};
use selectors::matching::matches_selector_list;
use selectors::parser::{
    ParseRelative, Parser as SelectorParserTrait, SelectorImpl, SelectorList,
    SelectorParseErrorKind,
};
use selectors::{Element, OpaqueElement};
#[cfg(test)]
use style::media_queries::MediaList;
#[cfg(test)]
use style::servo_arc::Arc;
#[cfg(test)]
use style::shared_lock::SharedRwLock;
#[cfg(test)]
use style::stylesheets::{AllowImportRules, Origin, Stylesheet};

use crate::language::is_supported_pseudo_class;
use hypreact_core::{ResolvedLayoutNode, RuntimeContentKind, RuntimeLayoutNodeType};

#[derive(Debug, Clone)]
pub struct LayoutDomTree {
    nodes: Vec<LayoutDomNode>,
    root: usize,
}

#[derive(Debug, Clone)]
struct LayoutDomNode {
    node: ResolvedLayoutNode,
    parent: Option<usize>,
    prev_sibling: Option<usize>,
    next_sibling: Option<usize>,
    first_child: Option<usize>,
}

#[derive(Clone, Copy, Debug)]
pub struct LayoutElement<'a> {
    tree: &'a LayoutDomTree,
    index: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LayoutSelectorImpl;

#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub struct LayoutAtom(String);

#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub struct LayoutAttrValue(String);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LayoutPseudoClass {
    Focused,
    Floating,
    Fullscreen,
    Urgent,
    Closing,
    EnterFromLeft,
    EnterFromRight,
    ExitToLeft,
    ExitToRight,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum LayoutPseudoElementStub {}

#[derive(Default)]
pub struct LayoutSelectorParser;

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum StyloAdapterError {
    #[error("invalid selector")]
    InvalidSelector,
}

impl ToCss for LayoutAtom {
    fn to_css<W>(&self, dest: &mut W) -> fmt::Result
    where
        W: fmt::Write,
    {
        serialize_identifier(&self.0, dest)
    }
}

impl ToCss for LayoutAttrValue {
    fn to_css<W>(&self, dest: &mut W) -> fmt::Result
    where
        W: fmt::Write,
    {
        use std::fmt::Write;
        dest.write_char('"')?;
        write!(cssparser::CssStringWriter::new(dest), "{}", &self.0)?;
        dest.write_char('"')
    }
}

impl ToCss for LayoutPseudoClass {
    fn to_css<W>(&self, dest: &mut W) -> fmt::Result
    where
        W: fmt::Write,
    {
        let name = match self {
            Self::Focused => "focused",
            Self::Floating => "floating",
            Self::Fullscreen => "fullscreen",
            Self::Urgent => "urgent",
            Self::Closing => "closing",
            Self::EnterFromLeft => "enter-from-left",
            Self::EnterFromRight => "enter-from-right",
            Self::ExitToLeft => "exit-to-left",
            Self::ExitToRight => "exit-to-right",
        };
        serialize_identifier(name, dest)
    }
}

impl<'a> From<&'a str> for LayoutAtom {
    fn from(value: &'a str) -> Self {
        Self(value.into())
    }
}

impl<'a> From<&'a str> for LayoutAttrValue {
    fn from(value: &'a str) -> Self {
        Self(value.into())
    }
}

impl AsRef<str> for LayoutAttrValue {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl PartialEq for LayoutElement<'_> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.tree, other.tree) && self.index == other.index
    }
}

impl Eq for LayoutElement<'_> {}

impl std::hash::Hash for LayoutElement<'_> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        (self.tree as *const LayoutDomTree).hash(state);
        self.index.hash(state);
    }
}

impl PrecomputedHash for LayoutAtom {
    fn precomputed_hash(&self) -> u32 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.0.hash(&mut hasher);
        hasher.finish() as u32
    }
}

impl selectors::parser::NonTSPseudoClass for LayoutPseudoClass {
    type Impl = LayoutSelectorImpl;

    fn is_active_or_hover(&self) -> bool {
        false
    }

    fn is_user_action_state(&self) -> bool {
        false
    }
}

impl SelectorImpl for LayoutSelectorImpl {
    type ExtraMatchingData<'a> = std::marker::PhantomData<&'a ()>;
    type AttrValue = LayoutAttrValue;
    type Identifier = LayoutAtom;
    type LocalName = LayoutAtom;
    type NamespaceUrl = LayoutAtom;
    type NamespacePrefix = LayoutAtom;
    type BorrowedNamespaceUrl = LayoutAtom;
    type BorrowedLocalName = LayoutAtom;
    type NonTSPseudoClass = LayoutPseudoClass;
    type PseudoElement = LayoutPseudoElementStub;
}

impl ToCss for LayoutPseudoElementStub {
    fn to_css<W>(&self, _dest: &mut W) -> fmt::Result
    where
        W: fmt::Write,
    {
        match *self {}
    }
}

impl selectors::parser::PseudoElement for LayoutPseudoElementStub {
    type Impl = LayoutSelectorImpl;
}

impl<'i> SelectorParserTrait<'i> for LayoutSelectorParser {
    type Impl = LayoutSelectorImpl;
    type Error = SelectorParseErrorKind<'i>;

    fn parse_non_ts_pseudo_class(
        &self,
        location: cssparser::SourceLocation,
        name: cssparser::CowRcStr<'i>,
    ) -> Result<LayoutPseudoClass, cssparser::ParseError<'i, SelectorParseErrorKind<'i>>> {
        let name = name.as_ref().to_ascii_lowercase();
        if !is_supported_pseudo_class(&name) {
            return Err(location.new_custom_error(
                SelectorParseErrorKind::UnsupportedPseudoClassOrElement(name.into()),
            ));
        }

        match name.as_str() {
            "focused" => Ok(LayoutPseudoClass::Focused),
            "floating" => Ok(LayoutPseudoClass::Floating),
            "fullscreen" => Ok(LayoutPseudoClass::Fullscreen),
            "urgent" => Ok(LayoutPseudoClass::Urgent),
            "closing" => Ok(LayoutPseudoClass::Closing),
            "enter-from-left" => Ok(LayoutPseudoClass::EnterFromLeft),
            "enter-from-right" => Ok(LayoutPseudoClass::EnterFromRight),
            "exit-to-left" => Ok(LayoutPseudoClass::ExitToLeft),
            "exit-to-right" => Ok(LayoutPseudoClass::ExitToRight),
            _ => unreachable!("supported pseudo-class must map to enum variant"),
        }
    }
}

impl LayoutDomTree {
    pub fn from_resolved_root(root: &ResolvedLayoutNode) -> Self {
        let mut tree = Self { nodes: Vec::new(), root: 0 };
        tree.root = tree.push_node(root.clone(), None, None);
        tree
    }

    pub fn root_element(&self) -> LayoutElement<'_> {
        LayoutElement { tree: self, index: self.root }
    }

    #[cfg(test)]
    pub fn find_window_element(
        &self,
        window_id: &hypreact_core::WindowId,
    ) -> Option<LayoutElement<'_>> {
        self.nodes.iter().enumerate().find_map(|(index, node)| match &node.node {
            ResolvedLayoutNode::Window { window_id: Some(id), .. } if id == window_id => {
                Some(LayoutElement { tree: self, index })
            }
            _ => None,
        })
    }

    fn push_node(
        &mut self,
        node: ResolvedLayoutNode,
        parent: Option<usize>,
        prev_sibling: Option<usize>,
    ) -> usize {
        let index = self.nodes.len();
        self.nodes.push(LayoutDomNode {
            node: node.clone(),
            parent,
            prev_sibling,
            next_sibling: None,
            first_child: None,
        });

        let children = match node {
            ResolvedLayoutNode::Workspace { children, .. }
            | ResolvedLayoutNode::Group { children, .. }
            | ResolvedLayoutNode::Window { children, .. }
            | ResolvedLayoutNode::Content { children, .. } => children,
        };

        let mut prev_child = None;
        for child in children {
            let child_index = self.push_node(child, Some(index), prev_child);
            if prev_child.is_none() {
                self.nodes[index].first_child = Some(child_index);
            }
            if let Some(prev_child_index) = prev_child {
                self.nodes[prev_child_index].next_sibling = Some(child_index);
            }
            prev_child = Some(child_index);
        }

        index
    }
}

impl<'a> LayoutElement<'a> {
    fn node(&self) -> &'a LayoutDomNode {
        &self.tree.nodes[self.index]
    }

    fn local_name_str(&self) -> &'static str {
        match &self.node().node {
            ResolvedLayoutNode::Content { meta, kind, .. } => {
                if let Some(name) = meta.name.as_deref() {
                    return Box::leak(name.to_string().into_boxed_str());
                }
                match kind {
                    RuntimeContentKind::Container => "content",
                    RuntimeContentKind::Text => "text",
                }
            }
            _ => match self.node().node.node_type() {
                RuntimeLayoutNodeType::Workspace => "workspace",
                RuntimeLayoutNodeType::Group => "group",
                RuntimeLayoutNodeType::Window => "window",
                RuntimeLayoutNodeType::Content => "content",
            },
        }
    }
}

impl<'a> Element for LayoutElement<'a> {
    type Impl = LayoutSelectorImpl;

    fn opaque(&self) -> OpaqueElement {
        OpaqueElement::new(&self.tree.nodes[self.index])
    }

    fn parent_element(&self) -> Option<Self> {
        self.node().parent.map(|index| Self { tree: self.tree, index })
    }

    fn parent_node_is_shadow_root(&self) -> bool {
        false
    }
    fn containing_shadow_host(&self) -> Option<Self> {
        None
    }
    fn is_pseudo_element(&self) -> bool {
        false
    }

    fn prev_sibling_element(&self) -> Option<Self> {
        self.node().prev_sibling.map(|index| Self { tree: self.tree, index })
    }

    fn next_sibling_element(&self) -> Option<Self> {
        self.node().next_sibling.map(|index| Self { tree: self.tree, index })
    }

    fn first_element_child(&self) -> Option<Self> {
        self.node().first_child.map(|index| Self { tree: self.tree, index })
    }

    fn is_html_element_in_html_document(&self) -> bool {
        false
    }

    fn has_local_name(&self, local_name: &LayoutAtom) -> bool {
        self.local_name_str() == local_name.0
    }

    fn has_namespace(&self, ns: &LayoutAtom) -> bool {
        ns.0.is_empty()
    }

    fn is_same_type(&self, other: &Self) -> bool {
        self.local_name_str() == other.local_name_str()
    }

    fn attr_matches(
        &self,
        ns: &NamespaceConstraint<&LayoutAtom>,
        local_name: &LayoutAtom,
        operation: &AttrSelectorOperation<&LayoutAttrValue>,
    ) -> bool {
        let namespace_ok = match ns {
            NamespaceConstraint::Any => true,
            NamespaceConstraint::Specific(namespace) => namespace.0.is_empty(),
        };
        if !namespace_ok {
            return false;
        }
        let meta = self.node().node.meta();
        let value = match local_name.0.as_str() {
            "id" => meta.id.as_deref(),
            name => meta.data.get(name).map(String::as_str),
        };
        value.is_some_and(|value| operation.eval_str(value))
    }

    fn match_non_ts_pseudo_class(
        &self,
        pc: &LayoutPseudoClass,
        _context: &mut MatchingContext<LayoutSelectorImpl>,
    ) -> bool {
        let class_name = match pc {
            LayoutPseudoClass::Focused => "focused",
            LayoutPseudoClass::Floating => "floating",
            LayoutPseudoClass::Fullscreen => "fullscreen",
            LayoutPseudoClass::Urgent => "urgent",
            LayoutPseudoClass::Closing => "closing",
            LayoutPseudoClass::EnterFromLeft => "enter-from-left",
            LayoutPseudoClass::EnterFromRight => "enter-from-right",
            LayoutPseudoClass::ExitToLeft => "exit-to-left",
            LayoutPseudoClass::ExitToRight => "exit-to-right",
        };

        self.node().node.meta().class.iter().any(|class| class == class_name)
    }

    fn apply_selector_flags(&self, _flags: selectors::matching::ElementSelectorFlags) {}
    fn is_link(&self) -> bool {
        false
    }
    fn is_html_slot_element(&self) -> bool {
        false
    }

    fn has_id(&self, id: &LayoutAtom, case_sensitivity: CaseSensitivity) -> bool {
        self.node()
            .node
            .meta()
            .id
            .as_deref()
            .is_some_and(|value| case_sensitivity.eq(value.as_bytes(), id.0.as_bytes()))
    }

    fn has_class(&self, name: &LayoutAtom, case_sensitivity: CaseSensitivity) -> bool {
        self.node()
            .node
            .meta()
            .class
            .iter()
            .any(|class| case_sensitivity.eq(class.as_bytes(), name.0.as_bytes()))
    }

    fn match_pseudo_element(
        &self,
        _pe: &LayoutPseudoElementStub,
        _context: &mut MatchingContext<LayoutSelectorImpl>,
    ) -> bool {
        false
    }

    fn has_custom_state(&self, _name: &LayoutAtom) -> bool {
        false
    }
    fn imported_part(&self, _name: &LayoutAtom) -> Option<LayoutAtom> {
        None
    }
    fn is_part(&self, _name: &LayoutAtom) -> bool {
        false
    }
    fn is_empty(&self) -> bool {
        self.node().first_child.is_none()
    }
    fn is_root(&self) -> bool {
        self.node().parent.is_none()
    }
    fn add_element_unique_hashes(&self, _filter: &mut selectors::bloom::BloomFilter) -> bool {
        false
    }
}

#[doc(hidden)]
pub fn parse_selector_list(
    input: &str,
) -> Result<SelectorList<LayoutSelectorImpl>, StyloAdapterError> {
    let parser = LayoutSelectorParser;
    let mut input = cssparser::ParserInput::new(input);
    let mut parser_input = cssparser::Parser::new(&mut input);
    parse_selector_list_from_parser(&parser, &mut parser_input)
}

pub fn parse_selector_list_from_parser<'i, 't>(
    parser: &LayoutSelectorParser,
    input: &mut cssparser::Parser<'i, 't>,
) -> Result<SelectorList<LayoutSelectorImpl>, StyloAdapterError> {
    SelectorList::parse(parser, input, ParseRelative::No)
        .map_err(|_| StyloAdapterError::InvalidSelector)
}

pub fn selector_matches_element(
    selector: &SelectorList<LayoutSelectorImpl>,
    element: LayoutElement<'_>,
) -> bool {
    let mut caches = SelectorCaches::default();
    let mut context = MatchingContext::new(
        MatchingMode::Normal,
        None,
        &mut caches,
        QuirksMode::NoQuirks,
        NeedsSelectorFlags::No,
        MatchingForInvalidation::No,
    );
    matches_selector_list(selector, &element, &mut context)
}

#[cfg(test)]
#[derive(Debug, Clone)]
pub struct StyloStylesheet {
    pub stylesheet: Stylesheet,
}

#[cfg(test)]
pub fn parse_stylesheet_with_stylo(source: &str) -> Result<StyloStylesheet, StyloAdapterError> {
    let lock = SharedRwLock::new();
    let media = Arc::new(lock.wrap(MediaList::empty()));
    let stylesheet = Stylesheet::from_str(
        source,
        url::Url::parse("about:blank").unwrap().into(),
        Origin::Author,
        media,
        lock,
        None,
        None,
        style::context::QuirksMode::NoQuirks,
        AllowImportRules::No,
    );

    Ok(StyloStylesheet { stylesheet })
}

#[cfg(test)]
pub fn compute_stylo_for_layout_tree(
    root: &ResolvedLayoutNode,
    stylesheet: &StyloStylesheet,
) -> Result<(), StyloAdapterError> {
    let tree = LayoutDomTree::from_resolved_root(root);
    let _ = tree.root_element();
    let _ = &stylesheet.stylesheet;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use hypreact_core::LayoutNodeMeta;
    use hypreact_core::WindowId;

    fn tree() -> LayoutDomTree {
        LayoutDomTree::from_resolved_root(&ResolvedLayoutNode::Workspace {
            meta: LayoutNodeMeta { id: Some("root".into()), ..LayoutNodeMeta::default() },
            children: vec![ResolvedLayoutNode::Group {
                meta: LayoutNodeMeta {
                    id: Some("stack".into()),
                    class: vec!["main".into()],
                    ..LayoutNodeMeta::default()
                },
                children: vec![ResolvedLayoutNode::Window {
                    meta: LayoutNodeMeta {
                        id: Some("win".into()),
                        class: vec!["focused".into()],
                        data: [("app_id".into(), "foot".into())].into_iter().collect(),
                        ..LayoutNodeMeta::default()
                    },
                    window_id: Some(WindowId::from("w1")),
                    children: Vec::new(),
                }],
            }],
        })
    }

    #[test]
    fn selector_adapter_matches_basic_type_class_and_attribute_selectors() {
        let tree = tree();
        let window = tree.find_window_element(&WindowId::from("w1")).unwrap();
        let selector = parse_selector_list("window.focused[app_id='foot']").unwrap();

        assert!(selector_matches_element(&selector, window));
    }

    #[test]
    fn selector_adapter_matches_descendant_relationships() {
        let tree = tree();
        let window = tree.find_window_element(&WindowId::from("w1")).unwrap();
        let selector = parse_selector_list("workspace #stack window").unwrap();

        assert!(selector_matches_element(&selector, window));
    }

    #[test]
    fn selector_adapter_matches_window_state_pseudo_selectors() {
        let tree = tree();
        let window = tree.find_window_element(&WindowId::from("w1")).unwrap();
        let selector = parse_selector_list("window:focused").unwrap();

        assert!(selector_matches_element(&selector, window));
    }

    #[test]
    fn stylo_layout_compute_entrypoint_is_callable() {
        let root = ResolvedLayoutNode::Workspace {
            meta: LayoutNodeMeta::default(),
            children: vec![ResolvedLayoutNode::Window {
                meta: LayoutNodeMeta::default(),
                window_id: Some(WindowId::from("w1")),
                children: Vec::new(),
            }],
        };
        let stylesheet = parse_stylesheet_with_stylo("window { width: 100%; }").unwrap();

        assert_eq!(compute_stylo_for_layout_tree(&root, &stylesheet), Ok(()));
    }
}
