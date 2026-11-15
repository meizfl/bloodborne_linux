//! Compatible save inspection and verified ZIP backups. No game save is mutated here.
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Read, Write},
    path::{Path, PathBuf},
};
type Result<T> = std::result::Result<T, String>;
const META: &str = "bbport-backup.json";
const LIMIT: u64 = 1024 * 1024 * 1024;
fn save_file(name: &str) -> bool {
    let Some(number) = name
        .strip_prefix("userdata")
        .or_else(|| name.strip_prefix("backup"))
    else {
        return false;
    };
    number.len() == 4 && number.bytes().all(|b| b.is_ascii_digit())
}
pub fn title(game: &Path) -> String {
    for folder in [
        game.join("sce_sys"),
        game.join("Sc0"),
        game.parent().unwrap_or(game).join("Sc0"),
    ] {
        if let Ok(bytes) = std::fs::read(folder.join("param.sfo")) {
            if let Ok(values) = crate::formats::sfo(&bytes) {
                let title = values
                    .get("INSTALL_DIR_SAVEDATA")
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty())
                    .or_else(|| values.get("TITLE_ID").and_then(Value::as_str))
                    .unwrap_or("")
                    .trim();
                if title.len() == 9
                    && title.as_bytes()[..4].iter().all(|c| c.is_ascii_uppercase())
                    && title.as_bytes()[4..].iter().all(|c| c.is_ascii_digit())
                {
                    return title.into();
                }
            }
        }
    }
    "CUSA00207".into()
}
pub fn target(user: &Path, game: &Path) -> PathBuf {
    user.join("savedata/1").join(title(game)).join("SPRJ0005")
}
fn name_character(c: char) -> bool {
    c.is_ascii_alphanumeric()
        || ('\u{c0}'..='\u{24f}').contains(&c)
        || ('\u{370}'..='\u{4ff}').contains(&c)
        || ('\u{3040}'..='\u{30ff}').contains(&c)
        || ('\u{4e00}'..='\u{9fff}').contains(&c)
        || ('\u{ac00}'..='\u{d7af}').contains(&c)
        || " _-.'!?".contains(c)
}
pub fn name_in(bytes: &[u8]) -> String {
    let words: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .collect();
    // The reference accepts the first 2..16-character candidate after four NULs,
    // then applies its name plausibility check (rather than trying later strings).
    for i in 4..words.len() {
        if words[i - 4..i] != [0, 0, 0, 0] {
            continue;
        }
        let mut chars = String::new();
        for &w in words.iter().skip(i).take(17) {
            if w == 0 {
                let n = chars.chars().count();
                if (2..=16).contains(&n) {
                    return if chars.trim() == chars
                        && n >= 3
                        && chars.chars().filter(|c| c.is_alphabetic()).count() >= 2
                    {
                        chars
                    } else {
                        String::new()
                    };
                }
                break;
            }
            let Some(c) = char::from_u32(w as u32) else {
                break;
            };
            if !name_character(c) {
                break;
            }
            chars.push(c);
        }
    }
    String::new()
}
fn head(path: &Path, size: usize) -> Result<Vec<u8>> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    file.take(size as u64)
        .read_to_end(&mut out)
        .map_err(|e| e.to_string())?;
    Ok(out)
}
pub fn characters(folder: &Path) -> Vec<String> {
    let profile = head(&folder.join("userdata0010"), 0x1096).unwrap_or_default();
    let flags = profile
        .get(0x108c..0x1096)
        .filter(|f| f.iter().all(|b| *b <= 1));
    (0..10)
        .filter(|i| flags.is_none_or(|f| f[*i] != 0))
        .filter_map(|i| {
            let data = head(&folder.join(format!("userdata{i:04}")), 0x20000).ok()?;
            if !data.iter().take(4096).any(|b| *b != 0) {
                return None;
            }
            let name = name_in(&data);
            if name.is_empty() {
                None
            } else {
                Some(name)
            }
        })
        .collect()
}
#[derive(Debug)]
pub struct Archive {
    pub saves: BTreeMap<String, String>,
    pub metadata: BTreeMap<String, String>,
    pub info: Option<Value>,
}
pub fn inspect_archive(path: &Path) -> Result<Archive> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    let mut saves = BTreeMap::new();
    let mut metadata = BTreeMap::new();
    let mut hashes = BTreeMap::new();
    let mut info = None;
    let mut total = 0u64;
    for i in 0..zip.len() {
        let mut member = zip.by_index(i).map_err(|e| e.to_string())?;
        if member.is_dir() {
            continue;
        }
        total = total
            .checked_add(member.size())
            .ok_or("Backup size overflow")?;
        if total > LIMIT {
            return Err("Backup exceeds the 1 GiB inspection limit".into());
        }
        let name = member.name().to_string();
        if hashes.contains_key(&name) {
            return Err("Backup contains duplicate member names".into());
        }
        let mut hash = Sha256::new();
        let mut bytes = Vec::new();
        let mut buffer = [0u8; 65536];
        loop {
            let n = member
                .read(&mut buffer)
                .map_err(|e| format!("Backup CRC/read error: {e}"))?;
            if n == 0 {
                break;
            }
            hash.update(&buffer[..n]);
            if name == META {
                if bytes.len() + n > 1024 * 1024 {
                    return Err("Backup metadata too large".into());
                }
                bytes.extend_from_slice(&buffer[..n]);
            }
        }
        hashes.insert(name.clone(), format!("{:x}", hash.finalize()));
        if name == META {
            let value: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            if !value.is_object() {
                return Err("Backup metadata is not an object".into());
            }
            info = Some(value);
            continue;
        }
        let normalized = name.replace('\\', "/");
        let parts: Vec<_> = normalized.split('/').collect();
        let leaf = *parts.last().unwrap_or(&"");
        let parent = parts
            .get(parts.len().saturating_sub(2))
            .copied()
            .unwrap_or("");
        let destination = if parent.to_lowercase().ends_with("sce_sys") {
            if !["param.bin", "icon0.png"].contains(&leaf) {
                continue;
            }
            &mut metadata
        } else {
            if !save_file(leaf) {
                continue;
            }
            &mut saves
        };
        if destination.insert(leaf.to_owned(), name).is_some() {
            return Err("Backup holds more than one save".into());
        }
    }
    if let Some(manifest) = info.as_ref().and_then(|v| v.get("sha256")) {
        let manifest = manifest
            .as_object()
            .ok_or("Backup SHA-256 manifest is not an object")?;
        let members: BTreeSet<_> = hashes.keys().filter(|n| n.as_str() != META).collect();
        if manifest.keys().collect::<BTreeSet<_>>() != members {
            return Err("Backup SHA-256 manifest is incomplete".into());
        }
        for (name, digest) in manifest {
            if digest.as_str() != hashes.get(name).map(String::as_str) {
                return Err(format!("Backup SHA-256 mismatch: {name}"));
            }
        }
    }
    if !saves.contains_key("userdata0010")
        || !(0..10).any(|i| saves.contains_key(&format!("userdata{i:04}")))
    {
        return Err("No complete save: profile and at least one character are required".into());
    }
    Ok(Archive {
        saves,
        metadata,
        info,
    })
}
fn collect(root: &Path, dir: &Path, out: &mut Vec<(String, PathBuf)>) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    let mut entries = std::fs::read_dir(dir)
        .map_err(|e| e.to_string())?
        .map(|e| e.map(|e| e.path()))
        .collect::<std::io::Result<Vec<_>>>()
        .map_err(|e| e.to_string())?;
    entries.sort();
    for p in entries {
        let m = std::fs::symlink_metadata(&p).map_err(|e| e.to_string())?;
        if m.file_type().is_symlink() {
            return Err("Save backup does not follow symlinks".into());
        }
        if m.is_dir() {
            collect(root, &p, out)?;
        } else if m.is_file() {
            out.push((
                p.strip_prefix(root)
                    .map_err(|e| e.to_string())?
                    .to_string_lossy()
                    .replace('\\', "/"),
                p,
            ));
        }
    }
    Ok(())
}
pub fn backup(
    user: &Path,
    game: &Path,
    reason: &str,
    label: &str,
    detail: &str,
) -> Result<Option<PathBuf>> {
    let current = target(user, game);
    if !current.is_dir()
        || !std::fs::read_dir(&current)
            .map_err(|e| e.to_string())?
            .filter_map(|e| e.ok())
            .any(|e| e.file_name().to_string_lossy().starts_with("userdata"))
    {
        return Ok(None);
    }
    let mut files = Vec::new();
    collect(user, &current, &mut files)?;
    collect(
        user,
        &current.with_file_name("SPRJ0005.sce_sys"),
        &mut files,
    )?;
    let now = chrono::Local::now();
    let characters = characters(&current);
    let mut info = json!({"version":1,"reason":reason,"label":label,"detail":detail,"created":now.format("%Y-%m-%dT%H:%M:%S").to_string(),"characters":characters});
    let words = if !label.is_empty() {
        label
    } else {
        match reason {
            "manual" => "Saved by hand",
            "auto" => "Before playing",
            "exit" => "After playing",
            "import" => "Before an import",
            "restore" => "Before a restore",
            "respec" => "Before a respec",
            other => other,
        }
    };
    let mut parts = vec![now.format("%Y-%m-%d %H.%M").to_string(), words.into()];
    if label.is_empty() && !detail.trim().is_empty() {
        parts.push(detail.trim().into());
    }
    if !characters.is_empty() {
        parts.push(characters.join(", "));
    }
    let text: String = parts
        .join(" - ")
        .chars()
        .map(|c| {
            if c.is_control() || "<>:\"/\\|?*".contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    let text = text.trim_matches([' ', '.']);
    let text: String = text.chars().take(116).collect();
    let text = text.trim_end_matches([' ', '.', '-']);
    let folder = user.join("save_backups");
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let mut target = folder.join(format!("{text}.zip"));
    let mut n = 1;
    while target.exists() {
        n += 1;
        target = folder.join(format!("{text} ({n}).zip"));
    }
    let staging = folder.join(format!(
        ".backup-{}-{}.part",
        std::process::id(),
        now.timestamp_nanos_opt().unwrap_or(0)
    ));
    let result = (|| -> Result<()> {
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staging)
            .map_err(|e| e.to_string())?;
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        let mut hashes = serde_json::Map::new();
        for (member, path) in files {
            let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
            hashes.insert(
                member.clone(),
                format!("{:x}", Sha256::digest(&bytes)).into(),
            );
            zip.start_file(member, options).map_err(|e| e.to_string())?;
            zip.write_all(&bytes).map_err(|e| e.to_string())?;
        }
        // A full manifest is compatible with legacy readers and verifies every copied byte.
        info["sha256"] = hashes.into();
        zip.start_file(META, options).map_err(|e| e.to_string())?;
        zip.write_all(&serde_json::to_vec_pretty(&info).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        let file = zip.finish().map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        drop(file);
        inspect_archive(&staging)?;
        std::fs::rename(&staging, &target).map_err(|e| e.to_string())?;
        Ok(())
    })();
    let _ = std::fs::remove_file(&staging);
    result?;
    Ok(Some(target))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn archive_rejects_a_false_hash_manifest() {
        let path =
            std::env::temp_dir().join(format!("bb-rust-corrupt-backup-{}.zip", std::process::id()));
        let file = std::fs::File::create(&path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        for name in [
            "savedata/1/CUSA00207/SPRJ0005/userdata0010",
            "savedata/1/CUSA00207/SPRJ0005/userdata0000",
        ] {
            zip.start_file(name, options).unwrap();
            zip.write_all(b"test data").unwrap();
        }
        zip.start_file(META, options).unwrap();
        zip.write_all(br#"{"sha256":{}}"#).unwrap();
        zip.finish().unwrap();
        assert!(inspect_archive(&path)
            .unwrap_err()
            .contains("manifest is incomplete"));
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn deleted_characters_are_not_listed_and_backups_verify() {
        let root = std::env::temp_dir().join(format!("bb-rust-save-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let save = target(&root, Path::new("/missing-game"));
        std::fs::create_dir_all(&save).unwrap();
        let mut profile = vec![0; 0x1096];
        profile[0x108c] = 1;
        std::fs::write(save.join("userdata0010"), profile).unwrap();
        let mut data = vec![0; 8];
        for w in "Hunter\0".encode_utf16() {
            data.extend_from_slice(&w.to_le_bytes());
        }
        std::fs::write(save.join("userdata0000"), &data).unwrap();
        std::fs::write(save.join("userdata0001"), &data).unwrap();
        assert_eq!(characters(&save), vec!["Hunter"]);
        let archive = backup(&root, Path::new("/missing-game"), "manual", "", "test")
            .unwrap()
            .unwrap();
        let verified = inspect_archive(&archive).unwrap();
        assert_eq!(verified.saves.len(), 3);
        assert!(verified.info.unwrap()["sha256"].is_object());
        assert_eq!(std::fs::read(save.join("userdata0000")).unwrap(), data);
        std::fs::remove_dir_all(root).unwrap();
    }
}
