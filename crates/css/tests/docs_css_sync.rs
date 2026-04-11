use std::fs;
use std::path::PathBuf;

use hypreact_css::language::{
    INVALID_SELECTOR_TARGET_NAMES, attribute_key_specs, property_specs, pseudo_class_specs,
    pseudo_element_specs,
};

fn css_docs() -> String {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let docs_path = manifest_dir.join("../../docs/css.md");
    fs::read_to_string(&docs_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", docs_path.display()))
}

#[test]
fn css_docs_cover_language_registry_surface() {
    let docs = css_docs();

    for property in property_specs() {
        assert!(
            docs.contains(&format!("`{}`", property.name)),
            "docs/css.md is missing property `{}`",
            property.name,
        );
    }

    for pseudo in pseudo_class_specs() {
        assert!(
            docs.contains(&format!("`:{}`", pseudo.name)),
            "docs/css.md is missing pseudo-class `:{}`",
            pseudo.name,
        );
    }

    for pseudo in pseudo_element_specs() {
        assert!(
            docs.contains(&format!("`window::{}`", pseudo.name)),
            "docs/css.md is missing pseudo-element `::{}`",
            pseudo.name,
        );
    }

    for attribute in attribute_key_specs() {
        assert!(
            docs.contains(&format!("`window[{}=\"...\"]`", attribute.name)),
            "docs/css.md is missing selector attribute key `{}`",
            attribute.name,
        );
    }

    for invalid_target in INVALID_SELECTOR_TARGET_NAMES {
        assert!(
            docs.contains(&format!("`{invalid_target}`")),
            "docs/css.md is missing invalid selector target `{invalid_target}`",
        );
    }
}
