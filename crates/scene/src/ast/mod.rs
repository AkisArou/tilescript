mod resolve;
mod validate;

pub use resolve::{LayoutResolveError, ResolvedLayoutTree};
pub use validate::{
    AuthoredLayoutNode, AuthoredNodeMeta, LayoutValidationError, ValidatedLayoutTree,
};

#[cfg(test)]
mod tests;
