#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CssDelimiter {
    Comma,
    Solidus,
    Semicolon,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CssDimension {
    pub value: f32,
    pub unit: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CssFunction {
    pub name: String,
    pub value: Vec<CssValueToken>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CssSimpleBlockKind {
    Bracket,
    Parenthesis,
    Brace,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CssSimpleBlock {
    pub kind: CssSimpleBlockKind,
    pub value: Vec<CssValueToken>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CssValueToken {
    Ident(String),
    String(String),
    Number(f32),
    Integer(i64),
    Dimension(CssDimension),
    Percentage(f32),
    Function(CssFunction),
    SimpleBlock(CssSimpleBlock),
    Delimiter(CssDelimiter),
    Whitespace,
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct CssValue {
    pub text: String,
    pub components: Vec<CssValueToken>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedDeclaration {
    pub property: String,
    pub value: CssValue,
}
