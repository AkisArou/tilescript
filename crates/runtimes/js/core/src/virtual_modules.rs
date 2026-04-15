pub fn source_for_virtual_module(specifier: &str) -> Option<&'static str> {
    match specifier {
        "@hypreact/sdk/api" => Some(include_str!("virtual/api.js")),
        "@hypreact/sdk/commands" => {
            Some(include_str!("../../../../../packages/sdk/js/src/commands.js"))
        }
        "@hypreact/sdk/config" => Some(include_str!("virtual/config.js")),
        "@hypreact/sdk/css.d.ts" => Some("export {}"),
        "@hypreact/sdk/jsx-runtime" => {
            Some(include_str!("../../../../../packages/sdk/js/src/jsx-runtime.js"))
        }
        "@hypreact/sdk/layout" => Some(include_str!("virtual/layout.js")),
        _ => None,
    }
}
