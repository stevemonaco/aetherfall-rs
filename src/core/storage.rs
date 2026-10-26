//! Small named text blobs (save slots, settings): files under the user's data folder natively, localStorage on the web.

#[cfg(not(target_arch = "wasm32"))]
fn path(key: &str) -> std::path::PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".local").join("share")))
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    base.join("Aetherfall").join("rs").join(format!("{key}.json"))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn read(key: &str) -> Option<String> {
    std::fs::read_to_string(path(key)).ok()
}

#[cfg(not(target_arch = "wasm32"))]
pub fn write(key: &str, text: &str) {
    let p = path(key);
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(p, text);
}

/// Appends one line to a `.jsonl` log beside the saves.
#[cfg(not(target_arch = "wasm32"))]
pub fn append_line(name: &str, line: &str) {
    use std::io::Write;
    let p = path(name).with_extension("jsonl");
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(p) {
        let _ = writeln!(file, "{line}");
    }
}

/// The web build keeps no logs.
#[cfg(target_arch = "wasm32")]
pub fn append_line(_name: &str, _line: &str) {}

#[cfg(target_arch = "wasm32")]
fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

#[cfg(target_arch = "wasm32")]
pub fn read(key: &str) -> Option<String> {
    local_storage()?.get_item(&format!("aetherfall_{key}")).ok()?
}

#[cfg(target_arch = "wasm32")]
pub fn write(key: &str, text: &str) {
    if let Some(storage) = local_storage() {
        let _ = storage.set_item(&format!("aetherfall_{key}"), text);
    }
}

pub fn exists(key: &str) -> bool {
    read(key).is_some()
}
