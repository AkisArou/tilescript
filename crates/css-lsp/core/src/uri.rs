use std::path::{Path, PathBuf};

use lsp_types::Url;

pub fn path_from_url(uri: &Url) -> Option<PathBuf> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        return uri.to_file_path().ok();
    }

    #[cfg(target_arch = "wasm32")]
    {
        if uri.scheme() != "file" {
            return None;
        }

        let mut path = uri.path().to_string();
        if cfg!(windows) && path.starts_with('/') {
            path.remove(0);
        }
        Some(PathBuf::from(percent_decode(&path)))
    }
}

pub fn url_from_path(path: &Path) -> Option<Url> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        return Url::from_file_path(path).ok();
    }

    #[cfg(target_arch = "wasm32")]
    {
        let path = path.to_string_lossy();
        let normalized = if path.starts_with('/') { path.to_string() } else { format!("/{path}") };
        Url::parse(&format!("file://{}", percent_encode_path(&normalized))).ok()
    }
}

#[cfg(target_arch = "wasm32")]
fn percent_decode(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let bytes = value.as_bytes();
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let hex = &value[index + 1..index + 3];
            if let Ok(decoded) = u8::from_str_radix(hex, 16) {
                output.push(decoded as char);
                index += 3;
                continue;
            }
        }

        output.push(bytes[index] as char);
        index += 1;
    }

    output
}

#[cfg(target_arch = "wasm32")]
fn percent_encode_path(value: &str) -> String {
    value
        .chars()
        .flat_map(|ch| match ch {
            ' ' => "%20".chars().collect::<Vec<_>>(),
            '#' => "%23".chars().collect::<Vec<_>>(),
            '?' => "%3F".chars().collect::<Vec<_>>(),
            '%' => "%25".chars().collect::<Vec<_>>(),
            _ => vec![ch],
        })
        .collect()
}
