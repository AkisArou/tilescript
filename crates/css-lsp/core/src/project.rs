use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use lsp_types::{Location, Range, Url};
use oxc::allocator::Allocator;
use oxc::ast::ast::{
    Expression, JSXAttributeItem, JSXAttributeName, JSXAttributeValue, JSXElementName, Program,
};
use oxc::parser::Parser;
use oxc::span::SourceType;

use crate::syntax::range_for;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProjectSelectorKind {
    Id,
    Class,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectSelectorMatch {
    pub kind: ProjectSelectorKind,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectSelectorSymbol {
    pub kind: ProjectSelectorKind,
    pub name: String,
    pub location: Location,
}

#[derive(Debug, Default, Clone)]
pub struct ProjectIndex {
    ids: BTreeSet<String>,
    classes: BTreeSet<String>,
    selector_files: HashMap<PathBuf, IndexedSelectorFile>,
    stylesheet_sources: HashMap<PathBuf, String>,
    app_scopes: HashMap<PathBuf, IndexedAppScope>,
    file_to_scopes: HashMap<PathBuf, BTreeSet<PathBuf>>,
}

#[derive(Debug, Default, Clone)]
struct IndexedAppScope {
    ids: BTreeSet<String>,
    classes: BTreeSet<String>,
    selector_files: BTreeSet<PathBuf>,
    stylesheets: BTreeSet<PathBuf>,
}

#[derive(Debug, Default, Clone)]
struct IndexedSelectorFile {
    ids: BTreeSet<String>,
    classes: BTreeSet<String>,
    selectors: Vec<IndexedSelectorOccurrence>,
}

#[derive(Debug, Clone)]
struct IndexedSelectorOccurrence {
    kind: ProjectSelectorKind,
    name: String,
    start: usize,
    end: usize,
    range: Range,
}

impl ProjectIndex {
    pub fn index_app_scope(
        &mut self,
        scope_id: PathBuf,
        script_sources: Vec<(PathBuf, String)>,
        stylesheet_sources: Vec<(PathBuf, String)>,
    ) {
        let mut scope = IndexedAppScope::default();

        for (path, source) in script_sources {
            let indexed = index_layout_source(&source);
            scope.ids.extend(indexed.ids.iter().cloned());
            scope.classes.extend(indexed.classes.iter().cloned());
            scope.selector_files.insert(path.clone());
            self.insert_selector_file(path, indexed, Some(&scope_id));
        }

        for (path, source) in stylesheet_sources {
            scope.stylesheets.insert(path.clone());
            self.stylesheet_sources.insert(path.clone(), source);
            self.file_to_scopes.entry(path).or_default().insert(scope_id.clone());
        }

        self.ids.extend(scope.ids.iter().cloned());
        self.classes.extend(scope.classes.iter().cloned());
        self.app_scopes.insert(scope_id, scope);
    }

    pub fn ids(&self) -> impl Iterator<Item = &String> {
        self.ids.iter()
    }

    pub fn classes(&self) -> impl Iterator<Item = &String> {
        self.classes.iter()
    }

    pub fn ids_for_path(&self, path: &Path) -> Vec<String> {
        self.scope_values_for_path(
            path,
            |scope| scope.ids.iter().cloned().collect(),
            || self.ids.iter().cloned().collect(),
        )
    }

    pub fn classes_for_path(&self, path: &Path) -> Vec<String> {
        self.scope_values_for_path(
            path,
            |scope| scope.classes.iter().cloned().collect(),
            || self.classes.iter().cloned().collect(),
        )
    }

    pub fn has_id_for_path(&self, path: &Path, id: &str) -> bool {
        self.scope_values_for_path(path, |scope| scope.ids.contains(id), || self.ids.contains(id))
    }

    pub fn has_class_for_path(&self, path: &Path, class_name: &str) -> bool {
        self.scope_values_for_path(
            path,
            |scope| scope.classes.contains(class_name),
            || self.classes.contains(class_name),
        )
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty() && self.classes.is_empty()
    }

    pub fn selector_definitions(&self, kind: ProjectSelectorKind, name: &str) -> Vec<Location> {
        self.selector_definitions_in_files(self.selector_files.keys(), kind, name)
    }

    pub fn selector_definitions_for_path(
        &self,
        path: &Path,
        kind: ProjectSelectorKind,
        name: &str,
    ) -> Vec<Location> {
        let files = self.scope_selector_files_for_path(path);
        if files.is_empty() {
            self.selector_definitions(kind, name)
        } else {
            self.selector_definitions_in_files(files.iter(), kind, name)
        }
    }

    pub fn selector_at(&self, path: &Path, offset: usize) -> Option<ProjectSelectorMatch> {
        self.selector_files.get(path)?.selectors.iter().find_map(|selector| {
            (selector.start <= offset && offset <= selector.end)
                .then(|| ProjectSelectorMatch { kind: selector.kind, name: selector.name.clone() })
        })
    }

    pub fn selector_rename_locations_for_path(
        &self,
        path: &Path,
        kind: ProjectSelectorKind,
        name: &str,
    ) -> Vec<Location> {
        self.selector_definitions_for_path(path, kind, name)
    }

    pub fn selector_symbols_matching(&self, query: &str) -> Vec<ProjectSelectorSymbol> {
        let normalized_query = query.trim().to_ascii_lowercase();
        let mut symbols = Vec::new();

        for (path, file) in &self.selector_files {
            let Some(uri) = crate::uri::url_from_path(path) else {
                continue;
            };

            for selector in &file.selectors {
                let candidate = match selector.kind {
                    ProjectSelectorKind::Id => format!("#{}", selector.name),
                    ProjectSelectorKind::Class => format!(".{}", selector.name),
                };

                if !normalized_query.is_empty()
                    && !selector.name.to_ascii_lowercase().contains(&normalized_query)
                    && !candidate.to_ascii_lowercase().contains(&normalized_query)
                {
                    continue;
                }

                symbols.push(ProjectSelectorSymbol {
                    kind: selector.kind,
                    name: selector.name.clone(),
                    location: Location { uri: uri.clone(), range: selector.range },
                });
            }
        }

        let mut seen = BTreeSet::new();
        symbols.retain(|symbol| {
            seen.insert((
                symbol.kind,
                symbol.name.clone(),
                symbol.location.uri.to_string(),
                symbol.location.range.start.line,
                symbol.location.range.start.character,
            ))
        });
        symbols
    }

    pub fn stylesheet_documents_for_path(&self, path: &Path) -> Vec<(Url, String)> {
        let stylesheet_paths = self.scope_stylesheets_for_path(path);
        let paths: Vec<_> =
            if stylesheet_paths.is_empty() && self.stylesheet_sources.contains_key(path) {
                vec![path.to_path_buf()]
            } else {
                stylesheet_paths.into_iter().collect()
            };

        paths
            .into_iter()
            .filter_map(|path| {
                let uri = crate::uri::url_from_path(&path)?;
                let source = self.stylesheet_sources.get(&path)?.clone();
                Some((uri, source))
            })
            .collect()
    }

    fn insert_selector_file(
        &mut self,
        path: PathBuf,
        indexed: IndexedSelectorFile,
        scope_id: Option<&PathBuf>,
    ) {
        self.ids.extend(indexed.ids.iter().cloned());
        self.classes.extend(indexed.classes.iter().cloned());
        self.selector_files.insert(path.clone(), indexed);
        if let Some(scope_id) = scope_id {
            self.file_to_scopes.entry(path).or_default().insert(scope_id.clone());
        }
    }

    fn scope_values_for_path<T>(
        &self,
        path: &Path,
        mut scoped: impl FnMut(&IndexedAppScope) -> T,
        fallback: impl FnOnce() -> T,
    ) -> T
    where
        T: ScopeValueMerge,
    {
        let Some(scope_ids) = self.file_to_scopes.get(path) else {
            return fallback();
        };

        let mut scope_iter = scope_ids.iter().filter_map(|scope_id| self.app_scopes.get(scope_id));
        let Some(first_scope) = scope_iter.next() else {
            return fallback();
        };

        let mut result = scoped(first_scope);
        for scope in scope_iter {
            result = merge_scope_values(result, scoped(scope));
        }
        result
    }

    fn selector_definitions_in_files<'a>(
        &self,
        files: impl Iterator<Item = &'a PathBuf>,
        kind: ProjectSelectorKind,
        name: &str,
    ) -> Vec<Location> {
        let mut locations = Vec::new();

        for path in files {
            let Some(uri) = crate::uri::url_from_path(path) else {
                continue;
            };
            let Some(file) = self.selector_files.get(path) else {
                continue;
            };
            locations.extend(file.selectors.iter().filter_map(|selector| {
                (selector.kind == kind && selector.name == name)
                    .then(|| Location { uri: uri.clone(), range: selector.range })
            }));
        }

        dedupe_locations(locations)
    }

    fn scope_selector_files_for_path(&self, path: &Path) -> BTreeSet<PathBuf> {
        let mut files = BTreeSet::new();
        for scope_id in self.file_to_scopes.get(path).into_iter().flatten() {
            if let Some(scope) = self.app_scopes.get(scope_id) {
                files.extend(scope.selector_files.iter().cloned());
            }
        }
        files
    }

    fn scope_stylesheets_for_path(&self, path: &Path) -> BTreeSet<PathBuf> {
        let mut files = BTreeSet::new();
        for scope_id in self.file_to_scopes.get(path).into_iter().flatten() {
            if let Some(scope) = self.app_scopes.get(scope_id) {
                files.extend(scope.stylesheets.iter().cloned());
            }
        }
        files
    }
}

fn merge_scope_values<T>(left: T, right: T) -> T
where
    T: ScopeValueMerge,
{
    left.merge(right)
}

trait ScopeValueMerge {
    fn merge(self, other: Self) -> Self;
}

impl ScopeValueMerge for bool {
    fn merge(self, other: Self) -> Self {
        self || other
    }
}

impl ScopeValueMerge for Vec<String> {
    fn merge(mut self, other: Self) -> Self {
        self.extend(other);
        self.sort();
        self.dedup();
        self
    }
}

fn dedupe_locations(locations: Vec<Location>) -> Vec<Location> {
    let mut seen = BTreeSet::new();
    let mut deduped = Vec::new();

    for location in locations {
        let key = (
            location.uri.to_string(),
            location.range.start.line,
            location.range.start.character,
            location.range.end.line,
            location.range.end.character,
        );
        if seen.insert(key) {
            deduped.push(location);
        }
    }

    deduped
}

fn index_layout_source(source: &str) -> IndexedSelectorFile {
    let allocator = Allocator::default();
    let source_type = SourceType::from_path("layout.tsx").unwrap_or_else(|_| SourceType::tsx());
    let parsed = Parser::new(&allocator, source, source_type).parse();

    let mut indexed = IndexedSelectorFile::default();
    walk_program(&parsed.program, source, &mut indexed);

    indexed
}

fn walk_program(program: &Program<'_>, source: &str, indexed: &mut IndexedSelectorFile) {
    for item in &program.body {
        walk_statement(item, source, indexed);
    }
}

fn walk_statement(
    statement: &oxc::ast::ast::Statement<'_>,
    source: &str,
    indexed: &mut IndexedSelectorFile,
) {
    use oxc::ast::ast::Statement;

    match statement {
        Statement::BlockStatement(block) => {
            for statement in &block.body {
                walk_statement(statement, source, indexed);
            }
        }
        Statement::ExpressionStatement(stmt) => walk_expression(&stmt.expression, source, indexed),
        Statement::ReturnStatement(stmt) => {
            if let Some(argument) = &stmt.argument {
                walk_expression(argument, source, indexed);
            }
        }
        Statement::IfStatement(stmt) => {
            walk_statement(&stmt.consequent, source, indexed);
            if let Some(alternate) = &stmt.alternate {
                walk_statement(alternate, source, indexed);
            }
        }
        Statement::VariableDeclaration(_) => {}
        Statement::FunctionDeclaration(func) => {
            if let Some(body) = &func.body {
                for statement in &body.statements {
                    walk_statement(statement, source, indexed);
                }
            }
        }
        Statement::ExportDefaultDeclaration(declaration) => match &declaration.declaration {
            oxc::ast::ast::ExportDefaultDeclarationKind::FunctionDeclaration(function) => {
                if let Some(body) = &function.body {
                    for statement in &body.statements {
                        walk_statement(statement, source, indexed);
                    }
                }
            }
            expression => {
                if let Some(expression) = expression.as_expression() {
                    walk_expression(expression, source, indexed);
                }
            }
        },
        _ => {}
    }
}

fn walk_expression(expression: &Expression<'_>, source: &str, indexed: &mut IndexedSelectorFile) {
    match expression {
        Expression::JSXElement(element) => walk_jsx_element(element, source, indexed),
        Expression::ConditionalExpression(expr) => {
            walk_expression(&expr.consequent, source, indexed);
            walk_expression(&expr.alternate, source, indexed);
        }
        Expression::LogicalExpression(expr) => {
            walk_expression(&expr.left, source, indexed);
            walk_expression(&expr.right, source, indexed);
        }
        Expression::ParenthesizedExpression(expr) => {
            walk_expression(&expr.expression, source, indexed)
        }
        _ => {}
    }
}

fn walk_jsx_element(
    element: &oxc::ast::ast::JSXElement<'_>,
    source: &str,
    indexed: &mut IndexedSelectorFile,
) {
    let Some(name) = jsx_name(&element.opening_element.name) else {
        return;
    };
    if !matches!(name, "workspace" | "group" | "window" | "slot") {
        return;
    }

    for attribute in &element.opening_element.attributes {
        let JSXAttributeItem::Attribute(attribute) = attribute else {
            continue;
        };
        let Some(attribute_name) = jsx_attribute_name(&attribute.name) else {
            continue;
        };

        match attribute_name {
            "id" => index_selector_attribute(
                indexed,
                source,
                attribute.value.as_ref(),
                ProjectSelectorKind::Id,
            ),
            "class" => index_selector_attribute(
                indexed,
                source,
                attribute.value.as_ref(),
                ProjectSelectorKind::Class,
            ),
            _ => {}
        }
    }

    for child in &element.children {
        match child {
            oxc::ast::ast::JSXChild::Element(child) => walk_jsx_element(child, source, indexed),
            oxc::ast::ast::JSXChild::ExpressionContainer(container) => {
                if let Some(expression) = container.expression.as_expression() {
                    walk_expression(expression, source, indexed);
                }
            }
            _ => {}
        }
    }
}

fn jsx_name<'a>(name: &'a JSXElementName<'a>) -> Option<&'a str> {
    match name {
        JSXElementName::Identifier(identifier) => Some(identifier.name.as_str()),
        _ => None,
    }
}

fn jsx_attribute_name<'a>(name: &'a JSXAttributeName<'a>) -> Option<&'a str> {
    match name {
        JSXAttributeName::Identifier(identifier) => Some(identifier.name.as_str()),
        _ => None,
    }
}

fn index_selector_attribute(
    indexed: &mut IndexedSelectorFile,
    source: &str,
    value: Option<&JSXAttributeValue<'_>>,
    kind: ProjectSelectorKind,
) {
    match kind {
        ProjectSelectorKind::Id => {
            let Some(value) = static_attribute_value_with_offsets(source, value) else {
                return;
            };
            let trimmed = value.text.trim();
            if trimmed.is_empty() {
                return;
            }
            let leading = value.text.len() - value.text.trim_start().len();
            let trailing = value.text.len() - value.text.trim_end().len();
            let start = value.start + leading;
            let end = value.end - trailing;
            push_selector_occurrence(indexed, source, kind, trimmed.to_string(), start, end);
        }
        ProjectSelectorKind::Class => {
            for segment in static_class_segments_with_offsets(source, value) {
                for (start, end, class_name) in split_class_segments(segment.text) {
                    push_selector_occurrence(
                        indexed,
                        source,
                        kind,
                        class_name.to_string(),
                        segment.start + start,
                        segment.start + end,
                    );
                }
            }
        }
    }
}

fn push_selector_occurrence(
    indexed: &mut IndexedSelectorFile,
    source: &str,
    kind: ProjectSelectorKind,
    name: String,
    start: usize,
    end: usize,
) {
    let Some(range) = range_for(source, start, end) else {
        return;
    };

    match kind {
        ProjectSelectorKind::Id => {
            indexed.ids.insert(name.clone());
        }
        ProjectSelectorKind::Class => {
            indexed.classes.insert(name.clone());
        }
    }

    indexed.selectors.push(IndexedSelectorOccurrence { kind, name, start, end, range });
}

struct StaticAttributeValue<'a> {
    text: &'a str,
    start: usize,
    end: usize,
}

fn static_attribute_value_with_offsets<'a>(
    source: &'a str,
    value: Option<&JSXAttributeValue<'_>>,
) -> Option<StaticAttributeValue<'a>> {
    match value? {
        JSXAttributeValue::StringLiteral(literal) => {
            source_segment_inside_delimiters(source, literal.span.start, literal.span.end)
        }
        JSXAttributeValue::ExpressionContainer(container) => {
            let expression = container.expression.as_expression()?;
            static_expression_value_with_offsets(source, expression)
        }
        _ => None,
    }
}

fn static_expression_value_with_offsets<'a>(
    source: &'a str,
    expression: &Expression<'_>,
) -> Option<StaticAttributeValue<'a>> {
    match expression {
        Expression::StringLiteral(literal) => {
            source_segment_inside_delimiters(source, literal.span.start, literal.span.end)
        }
        Expression::TemplateLiteral(template) if template.expressions.is_empty() => {
            source_segment_inside_delimiters(source, template.span.start, template.span.end)
        }
        _ => None,
    }
}

fn static_class_segments_with_offsets<'a>(
    source: &'a str,
    value: Option<&JSXAttributeValue<'_>>,
) -> Vec<StaticAttributeValue<'a>> {
    match value {
        Some(JSXAttributeValue::StringLiteral(literal)) => {
            source_segment_inside_delimiters(source, literal.span.start, literal.span.end)
                .into_iter()
                .collect()
        }
        Some(JSXAttributeValue::ExpressionContainer(container)) => {
            let Some(expression) = container.expression.as_expression() else {
                return Vec::new();
            };
            static_class_expression_segments(source, expression)
        }
        _ => Vec::new(),
    }
}

fn static_class_expression_segments<'a>(
    source: &'a str,
    expression: &Expression<'_>,
) -> Vec<StaticAttributeValue<'a>> {
    match expression {
        Expression::StringLiteral(_) | Expression::TemplateLiteral(_) => {
            static_expression_value_with_offsets(source, expression).into_iter().collect()
        }
        Expression::CallExpression(call) => call
            .arguments
            .iter()
            .filter_map(|argument| argument.as_expression())
            .flat_map(|argument| match argument {
                Expression::StringLiteral(_) | Expression::TemplateLiteral(_) => {
                    static_expression_value_with_offsets(source, argument).into_iter().collect()
                }
                _ => Vec::new(),
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn source_segment_inside_delimiters(
    source: &str,
    start: u32,
    end: u32,
) -> Option<StaticAttributeValue<'_>> {
    let start = usize::try_from(start).ok()?;
    let end = usize::try_from(end).ok()?;
    let content_start = start.checked_add(1)?;
    let content_end = end.checked_sub(1)?;
    (content_start <= content_end && content_end <= source.len()).then(|| StaticAttributeValue {
        text: &source[content_start..content_end],
        start: content_start,
        end: content_end,
    })
}

fn split_class_segments(input: &str) -> Vec<(usize, usize, &str)> {
    let mut segments = Vec::new();
    let bytes = input.as_bytes();
    let mut index = 0;

    while index < bytes.len() {
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        let start = index;
        while index < bytes.len() && !bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if start < index {
            segments.push((start, index, &input[start..index]));
        }
    }

    segments
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexes_static_ids_and_classes_from_layout_jsx() {
        let mut index = ProjectIndex::default();
        index.index_app_scope(
            PathBuf::from("/tmp/layouts/master-stack/index.tsx"),
            vec![(
                PathBuf::from("/tmp/layouts/master-stack/index.tsx"),
                r#"
            export default function layout() {
              return (
                <workspace id="root" class="workspace shell">
                  <group id="stack" class={`stack pane`}>
                    <slot id="master" class="main-slot" />
                  </group>
                </workspace>
              )
            }
            "#
                .to_string(),
            )],
            vec![(PathBuf::from("/tmp/layouts/master-stack/index.css"), String::new())],
        );

        assert!(index.ids().any(|id| id == "root"));
        assert!(index.ids().any(|id| id == "stack"));
        assert!(index.classes().any(|class| class == "workspace"));
        assert!(index.classes().any(|class| class == "pane"));
    }

    #[test]
    fn finds_selector_definition_locations_and_matches() {
        let path = PathBuf::from("/tmp/layouts/master-stack/index.tsx");
        let source = r#"export default function layout() { return <workspace id="root" class="shell main" /> }"#;
        let mut index = ProjectIndex::default();
        index.index_app_scope(
            path.clone(),
            vec![(path.clone(), source.to_string())],
            vec![(PathBuf::from("/tmp/layouts/master-stack/index.css"), String::new())],
        );

        let root_definitions = index.selector_definitions(ProjectSelectorKind::Id, "root");
        let shell_definitions = index.selector_definitions(ProjectSelectorKind::Class, "shell");
        let root_offset = source.find("root").unwrap();

        assert_eq!(root_definitions.len(), 1);
        assert_eq!(shell_definitions.len(), 1);
        assert_eq!(
            index.selector_at(&path, root_offset).unwrap(),
            ProjectSelectorMatch { kind: ProjectSelectorKind::Id, name: "root".to_string() }
        );
    }

    #[test]
    fn scopes_candidates_and_stylesheets_by_app() {
        let mut index = ProjectIndex::default();
        index.index_app_scope(
            PathBuf::from("/tmp/config.ts"),
            vec![(
                PathBuf::from("/tmp/config.tsx"),
                r#"export default function config() { return <workspace id="root" class="shell" /> }"#
                    .to_string(),
            )],
            vec![(PathBuf::from("/tmp/index.css"), "#root { color: red; }".to_string())],
        );

        assert_eq!(index.ids_for_path(Path::new("/tmp/index.css")), vec!["root".to_string()]);
        assert_eq!(index.classes_for_path(Path::new("/tmp/index.css")), vec!["shell".to_string()]);
        assert_eq!(index.stylesheet_documents_for_path(Path::new("/tmp/index.css")).len(), 1);
    }

    #[test]
    fn indexes_static_class_segments_from_helper_calls() {
        let mut index = ProjectIndex::default();
        index.index_app_scope(
            PathBuf::from("/tmp/layouts/master-stack/index.tsx"),
            vec![(
                PathBuf::from("/tmp/layouts/master-stack/index.tsx"),
                r#"
                export default function layout(weight: number) {
                  return <slot class={joinClasses("stack-group__item", growClass(weight))} />
                }
                "#
                .to_string(),
            )],
            vec![(PathBuf::from("/tmp/layouts/master-stack/index.css"), String::new())],
        );

        assert!(
            index
                .classes_for_path(Path::new("/tmp/layouts/master-stack/index.css"))
                .iter()
                .any(|class_name| class_name == "stack-group__item")
        );
        assert!(
            !index
                .classes_for_path(Path::new("/tmp/layouts/master-stack/index.css"))
                .iter()
                .any(|class_name| class_name.starts_with("grow-"))
        );
    }
}
