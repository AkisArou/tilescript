pub fn source_for_virtual_module(specifier: &str) -> Option<&'static str> {
    match specifier {
        "@tilescript/sdk/api" => Some(include_str!("virtual/api.js")),
        "@tilescript/sdk/commands" => {
            Some(include_str!("../../../../../packages/sdk/js/src/commands.js"))
        }
        "@tilescript/sdk/config" => Some(include_str!("virtual/config.js")),
        "@tilescript/sdk/css.d.ts" => Some("export {}"),
        "@tilescript/sdk/jsx-runtime" => {
            Some(include_str!("../../../../../packages/sdk/js/src/jsx-runtime.js"))
        }
        "@tilescript/sdk/layout" => Some(include_str!("virtual/layout.js")),
        _ => None,
    }
}
