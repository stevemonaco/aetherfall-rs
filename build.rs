//! Embeds the JSON content (maps, enemies, dialogue...) and lists the licensed images, so the web build knows what to
//! fetch without directory listings.

use std::fmt::Write as _;
use std::path::Path;

fn walk(dir: &Path, root: &Path, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(|e| e.path());
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, root, out);
        } else if let Ok(rel) = path.strip_prefix(root) {
            out.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
}

fn main() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let assets = Path::new(&manifest).join("assets");
    println!("cargo:rerun-if-changed=assets/data");
    println!("cargo:rerun-if-changed=assets/external");

    let mut files = Vec::new();
    walk(&assets.join("data"), &assets, &mut files);
    let mut code = String::from("pub static DATA_FILES: &[(&str, &str)] = &[\n");
    for f in files.iter().filter(|f| f.ends_with(".json")) {
        let full = assets.join(f).to_string_lossy().replace('\\', "/");
        writeln!(code, "    ({f:?}, include_str!({full:?})),").unwrap();
    }
    code.push_str("];\n");

    let mut external = Vec::new();
    walk(&assets.join("external"), &assets, &mut external);
    code.push_str("pub static IMAGE_FILES: &[&str] = &[\n");
    for f in external.iter().filter(|f| f.ends_with(".png")) {
        writeln!(code, "    {f:?},").unwrap();
    }
    code.push_str("];\n");

    let out = Path::new(&std::env::var("OUT_DIR").unwrap()).join("content.rs");
    std::fs::write(out, code).unwrap();

    // The commit the game was built from, for the balancing log; "+dirty" when there were uncommitted changes.
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/index");
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .args(args)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    };
    let mut commit = git(&["rev-parse", "--short", "HEAD"]).unwrap_or_else(|| "unknown".into());
    if git(&["status", "--porcelain", "--untracked-files=no"]).is_some_and(|s| !s.is_empty()) {
        commit.push_str("+dirty");
    }
    println!("cargo:rustc-env=GAME_COMMIT={commit}");
}
