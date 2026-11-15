use std::fs;
use std::path::{Path, PathBuf};
fn collect(root: &Path, directory: &Path, prefix: &str, files: &mut Vec<(String, PathBuf)>) {
    if !directory.exists() {
        return;
    }
    for entry in fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect(root, &path, prefix, files);
        } else {
            let name = format!(
                "{prefix}{}",
                path.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            );
            files.push((name, path.canonicalize().unwrap()));
        }
    }
}
fn main() {
    let root = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let mut files = Vec::new();
    collect(
        &root.join("resources"),
        &root.join("resources"),
        "",
        &mut files,
    );
    let fonts = root.join("../launcher/assets/fonts");
    collect(&fonts, &fonts, "fonts/", &mut files);
    println!("cargo:rerun-if-env-changed=BB_TK_BUNDLE");
    if let Some(directory) = std::env::var_os("BB_TK_BUNDLE") {
        let directory = PathBuf::from(directory);
        assert!(
            directory.join("lib").is_dir(),
            "BB_TK_BUNDLE must contain a staged toolkit"
        );
        collect(&directory, &directory, "toolkit/", &mut files);
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    let mut code = String::from("pub const ASSETS: &[(&str, &[u8])] = &[\n");
    for (name, file) in files {
        println!("cargo:rerun-if-changed={}", file.display());
        code.push_str(&format!(
            "({name:?}, include_bytes!({:?})),\n",
            file.to_string_lossy()
        ));
    }
    code.push_str("];\n");
    println!("cargo:rerun-if-changed=resources");
    fs::write(
        PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("assets.rs"),
        code,
    )
    .unwrap();
}
