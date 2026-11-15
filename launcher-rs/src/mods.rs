//! Loose asset mods. Source games and mod folders are always read only.
#[cfg(windows)]
mod junction;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
type Result<T> = std::result::Result<T, String>;
const FOLDERS: &[&str] = &[
    "action", "chr", "event", "facegen", "font", "map", "menu", "movie", "msg", "mtd", "obj",
    "other", "param", "paramdef", "parts", "remo", "script", "sfx", "shader", "sound",
];
const SERIALS: &[&str] = &[
    "",
    "app0",
    "CUSA03173",
    "CUSA00900",
    "CUSA00207",
    "CUSA00208",
    "CUSA00299",
    "CUSA01363",
    "CUSA03179",
    "CUSA03014",
    "CUSA03023",
];
fn entries(path: &Path) -> Result<Vec<PathBuf>> {
    let mut v = std::fs::read_dir(path)
        .map_err(|e| format!("{}: {e}", path.display()))?
        .map(|e| e.map(|e| e.path()))
        .collect::<std::io::Result<Vec<_>>>()
        .map_err(|e| e.to_string())?;
    v.sort();
    Ok(v)
}
fn child(path: &Path, name: &str) -> Result<PathBuf> {
    if path.is_dir() {
        for p in entries(path)? {
            if p.file_name().unwrap().to_string_lossy().to_lowercase() == name.to_lowercase() {
                return Ok(p);
            }
        }
    }
    Ok(path.join(name))
}
#[derive(Debug)]
pub struct Layout {
    pub root: PathBuf,
    pub prefix: bool,
}
pub fn layout(folder: &Path, depth: usize) -> Result<Option<Layout>> {
    if !folder.is_dir() {
        return Ok(None);
    }
    for wrapper in SERIALS {
        let base = if wrapper.is_empty() {
            folder.to_path_buf()
        } else {
            child(folder, wrapper)?
        };
        if child(&base, "dvdroot_ps4")?.is_dir() {
            return Ok(Some(Layout {
                root: base,
                prefix: false,
            }));
        }
    }
    let items: Vec<_> = entries(folder)?
        .into_iter()
        .filter(|p| !p.file_name().unwrap().to_string_lossy().starts_with('.'))
        .collect();
    let dirs: Vec<_> = items.iter().filter(|p| p.is_dir()).collect();
    if dirs.iter().any(|p| {
        FOLDERS.contains(
            &p.file_name()
                .unwrap()
                .to_string_lossy()
                .to_lowercase()
                .as_str(),
        )
    }) {
        return Ok(Some(Layout {
            root: folder.to_path_buf(),
            prefix: true,
        }));
    }
    let extras = [
        "", "txt", "md", "ini", "json", "pdf", "rtf", "nfo", "url", "htm", "html", "jpg", "jpeg",
        "png", "webp", "gif", "bmp",
    ];
    if depth < 3
        && dirs.len() == 1
        && !items.iter().any(|p| {
            p.is_file()
                && !extras.contains(
                    &p.extension()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_lowercase()
                        .as_str(),
                )
        })
    {
        return layout(dirs[0], depth + 1);
    }
    Ok(None)
}
pub fn discover(root: &Path) -> Result<Vec<String>> {
    if !root.is_dir() {
        return Ok(Vec::new());
    }
    let mut names = Vec::new();
    for p in entries(root)? {
        if p.is_dir() && layout(&p, 0)?.is_some() {
            names.push(p.file_name().unwrap().to_string_lossy().into_owned());
        }
    }
    names.sort_by_key(|n| (n.to_lowercase(), n.clone()));
    Ok(names)
}
pub fn selected(root: &Path, config: &Path) -> Result<Vec<String>> {
    let available = discover(root)?;
    if !config.is_file() {
        return Ok(available);
    }
    let raw = std::fs::read(config).map_err(|e| e.to_string())?;
    let settings: serde_json::Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    let list = |key: &str| -> Result<Vec<String>> {
        match settings.get(key) {
            None => Ok(Vec::new()),
            Some(v) => v
                .as_array()
                .ok_or_else(|| format!("Mod {key} must be a list"))?
                .iter()
                .map(|n| {
                    n.as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| format!("Mod {key} must contain folder names"))
                })
                .collect(),
        }
    };
    let disabled: BTreeSet<_> = list("disabled")?.into_iter().collect();
    let mut seen = BTreeSet::new();
    Ok(list("order")?
        .into_iter()
        .chain(available.iter().cloned())
        .filter(|n| available.contains(n) && !disabled.contains(n) && seen.insert(n.clone()))
        .collect())
}
pub fn files(folder: &Path) -> Result<Vec<(PathBuf, PathBuf)>> {
    let layout = layout(folder, 0)?.ok_or("Mod folder has no supported asset layout")?;
    let root = std::fs::canonicalize(&layout.root).map_err(|e| e.to_string())?;
    fn walk(
        root: &Path,
        dir: &Path,
        prefix: bool,
        out: &mut Vec<(PathBuf, PathBuf)>,
    ) -> Result<()> {
        for p in entries(dir)? {
            let meta = std::fs::symlink_metadata(&p).map_err(|e| e.to_string())?;
            if meta.file_type().is_symlink() {
                return Err(format!("Mod symlinks are unsupported: {}", p.display()));
            }
            if meta.is_dir() {
                walk(root, &p, prefix, out)?;
            } else if meta.is_file() {
                let relative = p.strip_prefix(root).map_err(|e| e.to_string())?;
                let first = relative
                    .components()
                    .next()
                    .ok_or("Empty mod path")?
                    .as_os_str()
                    .to_string_lossy()
                    .to_lowercase();
                if prefix && !FOLDERS.contains(&first.as_str()) {
                    continue;
                }
                if !prefix && first != "dvdroot_ps4" {
                    if ["eboot.bin", "sce_module", "sce_sys"].contains(&first.as_str()) {
                        return Err(format!(
                            "Executable/system replacement is unsupported: {}",
                            p.display()
                        ));
                    }
                    continue;
                }
                out.push((
                    if prefix {
                        Path::new("dvdroot_ps4").join(relative)
                    } else {
                        relative.to_path_buf()
                    },
                    p,
                ));
            } else {
                return Err(format!("Not a regular mod file: {}", p.display()));
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(&root, &root, layout.prefix, &mut out)?;
    Ok(out)
}
fn link(destination: &Path, source: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(source, destination).map_err(|e| e.to_string())
    }
    #[cfg(windows)]
    {
        if source.is_dir() {
            junction::create(source, destination)
        } else {
            std::fs::hard_link(source, destination)
                .or_else(|_| std::fs::copy(source, destination).map(|_| ()))
                .map_err(|e| e.to_string())
        }
    }
}
fn directory_link(metadata: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(unix)]
    {
        metadata.file_type().is_symlink()
    }
}
fn remove_directory_link(path: &Path) -> Result<()> {
    #[cfg(windows)]
    {
        std::fs::remove_dir(path).map_err(|e| e.to_string())
    }
    #[cfg(unix)]
    {
        std::fs::remove_file(path).map_err(|e| e.to_string())
    }
}
fn expand(directory: &Path) -> Result<()> {
    match std::fs::symlink_metadata(directory) {
        Ok(m) if directory_link(&m) => {
            let target = std::fs::canonicalize(directory).map_err(|e| e.to_string())?;
            if !target.is_dir() {
                return Err(format!("File/directory conflict: {}", directory.display()));
            }
            remove_directory_link(directory)?;
            std::fs::create_dir(directory).map_err(|e| e.to_string())?;
            for p in entries(&target)? {
                link(&directory.join(p.file_name().unwrap()), &p)?;
            }
        }
        Ok(m) if !m.is_dir() => {
            return Err(format!("File/directory conflict: {}", directory.display()))
        }
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            std::fs::create_dir(directory).map_err(|e| e.to_string())?
        }
        Err(e) => return Err(e.to_string()),
    }
    Ok(())
}
/// Owns a uniquely created temporary view. Cleanup never follows directory links.
pub struct Overlay {
    path: PathBuf,
    temporary: bool,
    pub conflicts: Vec<String>,
}
impl Overlay {
    pub fn path(&self) -> &Path {
        &self.path
    }
}
impl Drop for Overlay {
    fn drop(&mut self) {
        if self.temporary {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}
pub fn build(game: &Path, out: &Path, mods: &[(String, PathBuf)]) -> Result<Overlay> {
    let game = std::fs::canonicalize(game).map_err(|e| e.to_string())?;
    let mut replacements = Vec::new();
    let mut owners = BTreeMap::new();
    let mut conflicts = Vec::new();
    for (name, root) in mods {
        for (relative, source) in files(root)? {
            let key = relative.to_string_lossy().to_lowercase();
            if let Some(old) = owners.insert(key, name.clone()) {
                conflicts.push(format!("{}: {old} -> {name}", relative.display()));
            }
            replacements.push((relative, source));
        }
    }
    if replacements.is_empty() {
        return Ok(Overlay {
            path: game,
            temporary: false,
            conflicts,
        });
    }
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    #[cfg(windows)]
    let placement = {
        let canonical_out = std::fs::canonicalize(out).map_err(|e| e.to_string())?;
        let prefix = |p: &Path| {
            p.components()
                .next()
                .map(|c| c.as_os_str().to_string_lossy().to_lowercase())
        };
        if prefix(&canonical_out) != prefix(&game) {
            game.parent()
                .ok_or("Game has no parent directory")?
                .to_path_buf()
        } else {
            canonical_out
        }
    };
    #[cfg(windows)]
    let out = placement.as_path();
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let mut path = None;
    for n in 0..100 {
        let p = out.join(format!("mod-game-{}-{stamp}-{n}", std::process::id()));
        match std::fs::create_dir(&p) {
            Ok(()) => {
                path = Some(p);
                break;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e.to_string()),
        }
    }
    let overlay = Overlay {
        path: path.ok_or("Cannot allocate unique mod view")?,
        temporary: true,
        conflicts,
    };
    for p in entries(&game)? {
        link(&overlay.path.join(p.file_name().unwrap()), &p)?;
    }
    for (relative, source) in replacements {
        let mut parent = overlay.path.clone();
        let parts: Vec<_> = relative.components().collect();
        for part in &parts[..parts.len() - 1] {
            parent = child(&parent, &part.as_os_str().to_string_lossy())?;
            expand(&parent)?;
        }
        let dest = child(
            &parent,
            &parts.last().unwrap().as_os_str().to_string_lossy(),
        )?;
        if let Ok(m) = std::fs::symlink_metadata(&dest) {
            if m.file_type().is_symlink() || m.is_file() {
                std::fs::remove_file(&dest).map_err(|e| e.to_string())?;
            } else {
                return Err(format!("File/directory conflict: {}", relative.display()));
            }
        }
        link(&dest, &source)?;
    }
    Ok(overlay)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn overlay_precedence_case_matching_and_cleanup_preserve_sources() {
        let root = std::env::temp_dir().join(format!("bb-rust-mod-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let game = root.join("game");
        let m1 = root.join("mods/one");
        let m2 = root.join("mods/two");
        for p in [
            &game.join("dvdroot_ps4/chr"),
            &m1.join("CHR"),
            &m2.join("chr"),
        ] {
            std::fs::create_dir_all(p).unwrap();
        }
        std::fs::write(game.join("dvdroot_ps4/chr/player.bin"), b"original").unwrap();
        std::fs::create_dir_all(game.join("sce_sys")).unwrap();
        std::fs::write(game.join("sce_sys/untouched.bin"), b"keep me").unwrap();
        std::fs::write(m1.join("CHR/PLAYER.bin"), b"first").unwrap();
        std::fs::write(m2.join("chr/player.bin"), b"last").unwrap();
        let mods = vec![("one".into(), m1.clone()), ("two".into(), m2.clone())];
        let view = build(&game, &root.join("out"), &mods).unwrap();
        let path = view.path().to_path_buf();
        assert_eq!(
            std::fs::read(path.join("dvdroot_ps4/chr/player.bin")).unwrap(),
            b"last"
        );
        assert_eq!(view.conflicts.len(), 1);
        drop(view);
        assert!(!path.exists());
        assert_eq!(
            std::fs::read(game.join("sce_sys/untouched.bin")).unwrap(),
            b"keep me"
        );
        assert_eq!(
            std::fs::read(game.join("dvdroot_ps4/chr/player.bin")).unwrap(),
            b"original"
        );
        assert_eq!(std::fs::read(m1.join("CHR/PLAYER.bin")).unwrap(), b"first");
        std::fs::remove_dir_all(root).unwrap();
    }
}
