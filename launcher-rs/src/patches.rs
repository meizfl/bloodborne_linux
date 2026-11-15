//! shadPS4/GoldHEN literal patch compiler. Mask/scan patches are deliberately rejected.
use crate::formats::Elf;
use quick_xml::{
    events::{BytesStart, Event},
    Reader,
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
type Result<T> = std::result::Result<T, String>;
const BASE: u64 = 0x400000;
#[derive(Clone, Debug)]
pub struct Patch {
    pub name: String,
    pub version: String,
    pub executable: String,
    pub lines: Vec<BTreeMap<String, String>>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Write {
    pub offset: u64,
    pub data: Vec<u8>,
}
fn attributes(e: &BytesStart<'_>, reader: &Reader<&[u8]>) -> Result<BTreeMap<String, String>> {
    e.attributes()
        .map(|a| {
            let a = a.map_err(|e| e.to_string())?;
            Ok((
                std::str::from_utf8(a.key.as_ref())
                    .map_err(|e| e.to_string())?
                    .to_owned(),
                a.decode_and_unescape_value(reader.decoder())
                    .map_err(|e| e.to_string())?
                    .into_owned(),
            ))
        })
        .collect()
}
pub fn parse(xml: &str) -> Result<Vec<Patch>> {
    let mut reader = Reader::from_str(xml);
    let mut patches = Vec::new();
    let mut current = None;
    loop {
        match reader.read_event().map_err(|e| e.to_string())? {
            Event::Start(e) | Event::Empty(e) if e.name().as_ref() == b"Metadata" => {
                let a = attributes(&e, &reader)?;
                patches.push(Patch {
                    name: a.get("Name").cloned().unwrap_or_default(),
                    version: a.get("AppVer").cloned().unwrap_or_default(),
                    executable: a
                        .get("AppElf")
                        .cloned()
                        .unwrap_or_else(|| "eboot.bin".into()),
                    lines: Vec::new(),
                });
                current = Some(patches.len() - 1);
            }
            Event::Start(e) | Event::Empty(e) if e.name().as_ref() == b"Line" => {
                if let Some(i) = current {
                    patches[i].lines.push(attributes(&e, &reader)?);
                }
            }
            Event::End(e) if e.name().as_ref() == b"Metadata" => current = None,
            Event::DocType(_) => {
                return Err("XML document type declarations are unsupported".into())
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(patches)
}
fn integer(s: &str) -> Result<u64> {
    let s = s.trim();
    let (radix, s) = if let Some(s) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        (16, s)
    } else if let Some(s) = s.strip_prefix("0b").or_else(|| s.strip_prefix("0B")) {
        (2, s)
    } else if let Some(s) = s.strip_prefix("0o").or_else(|| s.strip_prefix("0O")) {
        (8, s)
    } else {
        (10, s)
    };
    u64::from_str_radix(s, radix).map_err(|e| e.to_string())
}
pub fn encode(line: &BTreeMap<String, String>) -> Result<Vec<u8>> {
    let kind = line.get("Type").ok_or("Missing patch type")?.as_str();
    let value = line.get("Value").ok_or("Missing patch value")?;
    match kind {
        "bytes" => {
            let s: String = value.chars().filter(|c| !c.is_ascii_whitespace()).collect();
            if !s.is_ascii() || s.len() % 2 != 0 {
                return Err("Invalid patch hex".into());
            }
            (0..s.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|e| e.to_string()))
                .collect()
        }
        "bytes16" | "bytes32" | "bytes64" => {
            let size = kind[5..].parse::<usize>().map_err(|e| e.to_string())? / 8;
            let n = integer(value)?;
            if size < 8 && n >= 1u64 << (size * 8) {
                return Err("Patch integer overflow".into());
            }
            Ok(n.to_le_bytes()[..size].to_vec())
        }
        "float32" => Ok(value
            .parse::<f32>()
            .map_err(|e| e.to_string())?
            .to_le_bytes()
            .to_vec()),
        "float64" => Ok(value
            .parse::<f64>()
            .map_err(|e| e.to_string())?
            .to_le_bytes()
            .to_vec()),
        "utf8" => {
            let mut data = value.as_bytes().to_vec();
            data.push(0);
            Ok(data)
        }
        "utf16" => {
            let mut data: Vec<u8> = value.encode_utf16().flat_map(u16::to_le_bytes).collect();
            data.extend_from_slice(&[0, 0]);
            Ok(data)
        }
        _ => Err(format!("Unsupported patch type: {kind}")),
    }
}
pub fn compile(
    patches: &[Patch],
    names: &[String],
    version: &str,
    elf: &Elf,
) -> Result<Vec<Write>> {
    let mut found = BTreeMap::new();
    for p in patches {
        if p.version == version && p.executable == "eboot.bin" {
            found.insert(p.name.as_str(), p);
        }
    }
    let mut writes = Vec::new();
    for name in names {
        let patch = found
            .get(name.as_str())
            .ok_or_else(|| format!("Patch not found for {version}: {name}"))?;
        for line in &patch.lines {
            let address = integer(line.get("Address").ok_or("Missing patch address")?)?;
            let offset = address.checked_sub(BASE).ok_or("Patch before eboot base")?;
            let data = encode(line)?;
            let end = offset
                .checked_add(data.len() as u64)
                .ok_or("Patch range overflow")?;
            if !elf
                .segments
                .iter()
                .any(|s| s.kind == 1 && s.address <= offset && end <= s.address + s.memory_size)
            {
                return Err(format!(
                    "{name}: patch address {address:#x} is outside the eboot"
                ));
            }
            writes.push(Write { offset, data });
        }
    }
    Ok(writes)
}
pub fn blob(writes: &[Write]) -> Vec<u8> {
    let mut out = b"BBPATCH2".to_vec();
    out.extend_from_slice(&BASE.to_le_bytes());
    out.extend_from_slice(&(writes.len() as u64).to_le_bytes());
    for w in writes {
        out.extend_from_slice(&w.offset.to_le_bytes());
        out.extend_from_slice(&(w.data.len() as u64).to_le_bytes());
        out.extend_from_slice(&w.data);
    }
    out
}
/// Hash original executable bytes over the union of patch ranges, in address order.
pub fn fingerprint(writes: &[Write], elf: &Elf) -> Result<String> {
    let mut ranges: Vec<_> = writes
        .iter()
        .map(|w| (w.offset, w.offset + w.data.len() as u64))
        .collect();
    ranges.sort_unstable();
    let mut merged = Vec::<(u64, u64)>::new();
    for (start, end) in ranges {
        if let Some(last) = merged.last_mut() {
            if start <= last.1 {
                last.1 = last.1.max(end);
                continue;
            }
        }
        merged.push((start, end));
    }
    let mut hash = Sha256::new();
    for (start, end) in merged {
        hash.update(start.to_le_bytes());
        hash.update(end.to_le_bytes());
        let mut bytes = vec![0; (end - start) as usize];
        for p in elf.segments.iter().filter(|p| p.kind == 1) {
            let low = start.max(p.address);
            let high = end.min(p.address + p.file_size);
            if low < high {
                bytes[(low - start) as usize..(high - start) as usize].copy_from_slice(
                    crate::formats::span(&elf.bytes, p.offset + low - p.address, high - low)?,
                );
            }
        }
        hash.update(bytes);
    }
    Ok(format!("{:x}", hash.finalize()))
}
pub fn names(patches: &[Patch], version: &str) -> BTreeSet<String> {
    patches
        .iter()
        .filter(|p| p.version == version && p.executable == "eboot.bin")
        .map(|p| p.name.clone())
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn literals_entities_and_unsupported_scans() {
        let p=parse(r#"<Patch><Metadata Name="A &amp; B" AppVer="01.09"><PatchList><Line Type="bytes32" Address="0x400010" Value="0x12345678"/><Line Type="utf16" Address="0x400020" Value="Охотник"/></PatchList></Metadata></Patch>"#).unwrap();
        assert_eq!(p[0].name, "A & B");
        assert_eq!(encode(&p[0].lines[0]).unwrap(), [0x78, 0x56, 0x34, 0x12]);
        assert_eq!(encode(&p[0].lines[1]).unwrap().len(), 16);
        let mut line = p[0].lines[0].clone();
        line.insert("Type".into(), "mask".into());
        assert!(encode(&line).is_err());
        line.insert("Type".into(), "bytes16".into());
        assert!(encode(&line).is_err());
    }
}
