//! Bounds-checked parsers for the existing plaintext PS4 SELF/ELF and SFO formats.
//! No source game file is ever modified by this layer.
use base64::{engine::general_purpose::STANDARD, Engine};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
type Result<T> = std::result::Result<T, String>;
const LIMIT: u64 = 512 * 1024 * 1024;

pub fn span(data: &[u8], offset: u64, size: u64) -> Result<&[u8]> {
    let end = offset.checked_add(size).ok_or("File offset overflow")?;
    if end > data.len() as u64 {
        return Err(format!(
            "Out of bounds: {offset:#x}+{size:#x} / {:#x}",
            data.len()
        ));
    }
    Ok(&data[offset as usize..end as usize])
}
pub fn u16_at(data: &[u8], at: u64) -> Result<u16> {
    Ok(u16::from_le_bytes(span(data, at, 2)?.try_into().unwrap()))
}
pub fn u32_at(data: &[u8], at: u64) -> Result<u32> {
    Ok(u32::from_le_bytes(span(data, at, 4)?.try_into().unwrap()))
}
pub fn u64_at(data: &[u8], at: u64) -> Result<u64> {
    Ok(u64::from_le_bytes(span(data, at, 8)?.try_into().unwrap()))
}
pub fn i64_at(data: &[u8], at: u64) -> Result<i64> {
    Ok(i64::from_le_bytes(span(data, at, 8)?.try_into().unwrap()))
}
pub fn c_string(data: &[u8], at: u64) -> Result<&str> {
    let data = span(
        data,
        at,
        (data.len() as u64)
            .checked_sub(at)
            .ok_or("String offset outside table")?,
    )?;
    let end = data
        .iter()
        .position(|b| *b == 0)
        .ok_or("Unterminated table string")?;
    std::str::from_utf8(&data[..end]).map_err(|e| e.to_string())
}
#[derive(Clone, Debug)]
pub struct Header {
    pub entry: u64,
    pub program_offset: u64,
    pub program_count: u16,
}
#[derive(Clone, Debug)]
pub struct Segment {
    pub kind: u32,
    pub flags: u32,
    pub offset: u64,
    pub address: u64,
    pub file_size: u64,
    pub memory_size: u64,
    pub alignment: u64,
}
impl Segment {
    pub fn loadable(&self) -> bool {
        self.kind == 1 || self.kind == 0x61000010
    }
    pub fn mapped(&self, address: u64, length: u64) -> bool {
        address >= self.address
            && address
                .checked_add(length)
                .zip(self.address.checked_add(self.memory_size))
                .map(|(end, limit)| end <= limit)
                .unwrap_or(false)
    }
}
#[derive(Clone, Debug)]
pub struct Elf {
    pub bytes: Vec<u8>,
    pub header: Header,
    pub segments: Vec<Segment>,
    pub missing: Vec<usize>,
}
impl Elf {
    pub fn parse(source: &[u8]) -> Result<Self> {
        let plain = span(source, 0, 4)? == b"\x7fELF";
        if !plain && span(source, 0, 4)? != b"O\x15=\x1d" {
            return Err("Expected a plaintext PS4 SELF or ELF".into());
        }
        let count = if plain { 0 } else { u16_at(source, 24)? };
        let base = if plain { 0 } else { 32 + u64::from(count) * 32 };
        if span(source, base, 7)? != b"\x7fELF\x02\x01\x01" || u16_at(source, base + 18)? != 62 {
            return Err("Expected little-endian x86-64 ELF".into());
        }
        let header = Header {
            entry: u64_at(source, base + 24)?,
            program_offset: u64_at(source, base + 32)?,
            program_count: u16_at(source, base + 56)?,
        };
        if u16_at(source, base + 54)? != 56
            || header.program_count == 0
            || header.program_count >= 256
        {
            return Err("Unsupported program headers".into());
        }
        let table_size = u64::from(header.program_count) * 56;
        span(
            source,
            base.checked_add(header.program_offset)
                .ok_or("Header offset overflow")?,
            table_size,
        )?;
        let mut segments = Vec::new();
        for i in 0..u64::from(header.program_count) {
            let p = base + header.program_offset + i * 56;
            segments.push(Segment {
                kind: u32_at(source, p)?,
                flags: u32_at(source, p + 4)?,
                offset: u64_at(source, p + 8)?,
                address: u64_at(source, p + 16)?,
                file_size: u64_at(source, p + 32)?,
                memory_size: u64_at(source, p + 40)?,
                alignment: u64_at(source, p + 48)?,
            });
        }
        let header_end = header
            .program_offset
            .checked_add(table_size)
            .ok_or("Header end overflow")?;
        let mut end = header_end;
        for p in &segments {
            if p.loadable() {
                let memory_end = p
                    .address
                    .checked_add(p.memory_size)
                    .ok_or("Memory segment address overflow")?;
                if memory_end > LIMIT || p.file_size > p.memory_size {
                    return Err("Invalid memory segment layout".into());
                }
            }
            end = end.max(
                p.offset
                    .checked_add(p.file_size)
                    .ok_or("Segment end overflow")?,
            );
        }
        if end > LIMIT {
            return Err("Probe image exceeds 512 MiB".into());
        }
        if plain {
            return Ok(Self {
                bytes: span(source, 0, end)?.to_vec(),
                header,
                segments,
                missing: Vec::new(),
            });
        }
        let mut bytes = vec![0; end as usize];
        bytes[..header_end as usize].copy_from_slice(span(source, base, header_end)?);
        let mut covered = vec![(0, header_end)];
        for i in 0..u64::from(count) {
            let at = 32 + i * 32;
            let flags = u64_at(source, at)?;
            if flags & 0x800 == 0 {
                continue;
            }
            if flags & 10 != 0 {
                return Err("Encrypted/compressed SELF segment is unsupported".into());
            }
            let index = ((flags >> 20) & 4095) as usize;
            let segment = segments.get(index).ok_or("Invalid SELF segment index")?;
            let offset = u64_at(source, at + 8)?;
            let size = u64_at(source, at + 16)?;
            let memory = u64_at(source, at + 24)?;
            if size != segment.file_size || size != memory {
                return Err("Unsupported blocked SELF segment layout".into());
            }
            let range = segment.offset as usize..(segment.offset + size) as usize;
            bytes[range].copy_from_slice(span(source, offset, size)?);
            covered.push((segment.offset, segment.offset + size));
        }
        let mut missing = Vec::new();
        for (i, p) in segments.iter().enumerate() {
            if p.file_size > 0
                && !covered
                    .iter()
                    .any(|(a, b)| *a <= p.offset && p.offset + p.file_size <= *b)
            {
                missing.push(i);
                if matches!(p.kind, 1 | 2 | 0x61000000 | 0x61000010) {
                    return Err(format!("Required segment {i} is unavailable"));
                }
            }
        }
        Ok(Self {
            bytes,
            header,
            segments,
            missing,
        })
    }
    pub fn segment(&self, kind: u32) -> Result<&Segment> {
        self.segments
            .iter()
            .find(|p| p.kind == kind)
            .ok_or_else(|| format!("Original-file table {kind:#x} is missing"))
    }
    pub fn dynamic(&self) -> Result<BTreeMap<u64, u64>> {
        let p = self.segment(2)?;
        if p.file_size % 16 != 0 {
            return Err("Invalid dynamic table size".into());
        }
        let mut tags = BTreeMap::new();
        for at in (p.offset..p.offset + p.file_size).step_by(16) {
            let tag = u64_at(&self.bytes, at)?;
            if tag == 0 {
                break;
            }
            tags.insert(tag, u64_at(&self.bytes, at + 8)?);
        }
        Ok(tags)
    }
    pub fn table(&self, offset: u64, size: u64) -> Result<&[u8]> {
        let tags = self.dynamic()?;
        let blob = self.segment(0x61000000)?;
        span(
            span(&self.bytes, blob.offset, blob.file_size)?,
            *tags
                .get(&offset)
                .ok_or_else(|| format!("Missing table {offset:#x}"))?,
            *tags
                .get(&size)
                .ok_or_else(|| format!("Missing table size {size:#x}"))?,
        )
    }
    pub fn mapped(&self, address: u64, length: u64) -> bool {
        self.segments
            .iter()
            .any(|p| p.loadable() && p.mapped(address, length))
    }
    pub fn image(&self) -> Result<Vec<u8>> {
        let mut size = 0;
        for p in self.segments.iter().filter(|p| p.loadable()) {
            size = size.max(
                p.address
                    .checked_add(p.memory_size)
                    .ok_or("Memory image overflow")?,
            );
        }
        if size > LIMIT || !self.mapped(self.header.entry, 1) {
            return Err("Invalid memory image".into());
        }
        let mut image = vec![0; size as usize];
        for p in self.segments.iter().filter(|p| p.loadable()) {
            if p.file_size > p.memory_size {
                return Err("Segment file size exceeds memory size".into());
            }
            image[p.address as usize..(p.address + p.file_size) as usize].copy_from_slice(span(
                &self.bytes,
                p.offset,
                p.file_size,
            )?);
        }
        Ok(image)
    }
    pub fn image_hash(&self) -> Result<Option<String>> {
        let mut loads: Vec<_> = self.segments.iter().filter(|p| p.loadable()).collect();
        loads.sort_by_key(|p| p.address);
        let mut digest = Sha256::new();
        let mut at = 0;
        let zero = [0u8; 8192];
        fn zeros(digest: &mut Sha256, mut count: u64, zero: &[u8]) {
            while count > 0 {
                let n = count.min(zero.len() as u64) as usize;
                digest.update(&zero[..n]);
                count -= n as u64;
            }
        }
        for p in &loads {
            if p.address < at {
                return Ok(None);
            }
            if p.address > LIMIT || p.memory_size > LIMIT {
                return Err("Memory image exceeds limit".into());
            }
            zeros(&mut digest, p.address - at, &zero);
            digest.update(span(&self.bytes, p.offset, p.file_size)?);
            at = p.address + p.file_size;
        }
        let end = loads
            .iter()
            .filter_map(|p| p.address.checked_add(p.memory_size))
            .max()
            .unwrap_or(0);
        if end > LIMIT {
            return Err("Memory image exceeds limit".into());
        }
        zeros(&mut digest, end.saturating_sub(at), &zero);
        Ok(Some(format!("{:x}", digest.finalize())))
    }
}

pub fn sfo(data: &[u8]) -> Result<BTreeMap<String, serde_json::Value>> {
    if span(data, 0, 4)? != b"\0PSF" {
        return Err("Invalid SFO signature".into());
    }
    let keys = u64::from(u32_at(data, 8)?);
    let values = u64::from(u32_at(data, 12)?);
    let count = u32_at(data, 16)?;
    span(data, 20, u64::from(count) * 16)?;
    let mut result = BTreeMap::new();
    for i in 0..u64::from(count) {
        let at = 20 + i * 16;
        let key = u64::from(u16_at(data, at)?);
        let kind = u16_at(data, at + 2)?;
        let size = u64::from(u32_at(data, at + 4)?);
        let offset = u64::from(u32_at(data, at + 12)?);
        let key = c_string(
            data,
            keys.checked_add(key).ok_or("SFO key offset overflow")?,
        )?
        .to_owned();
        let bytes = span(
            data,
            values
                .checked_add(offset)
                .ok_or("SFO value offset overflow")?,
            size,
        )?;
        let value = if kind == 0x404 {
            if bytes.len() != 4 {
                return Err("Invalid SFO integer".into());
            }
            serde_json::Value::from(u32_at(bytes, 0)?)
        } else {
            let end = bytes.iter().rposition(|b| *b != 0).map_or(0, |p| p + 1);
            serde_json::Value::from(String::from_utf8_lossy(&bytes[..end]).into_owned())
        };
        result.insert(key, value);
    }
    Ok(result)
}
pub fn nid(name: &str) -> Result<String> {
    if !name.is_ascii() {
        return Err("NID symbol must be ASCII".into());
    }
    let salt = [
        0x51, 0x8d, 0x64, 0xa6, 0x35, 0xde, 0xd8, 0xc1, 0xe6, 0xb0, 0x39, 0xb1, 0xc3, 0xe5, 0x52,
        0x30,
    ];
    let mut digest = sha1::Sha1::new();
    digest.update(name.as_bytes());
    digest.update(salt);
    let digest = digest.finalize();
    let bytes: Vec<u8> = digest[..8].iter().copied().rev().collect();
    Ok(STANDARD
        .encode(bytes)
        .trim_end_matches('=')
        .replace('/', "-"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malicious_offsets_fail_without_wrapping() {
        assert!(span(&[0; 4], u64::MAX, 8).is_err());
        assert!(c_string(b"abc", 0).is_err());
        assert!(sfo(&[]).is_err());
    }
    #[test]
    fn executable_memory_ranges_are_validated_before_patching() {
        let mut data = vec![0; 120];
        data[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
        data[18..20].copy_from_slice(&62u16.to_le_bytes());
        data[32..40].copy_from_slice(&64u64.to_le_bytes());
        data[54..56].copy_from_slice(&56u16.to_le_bytes());
        data[56..58].copy_from_slice(&1u16.to_le_bytes());
        data[64..68].copy_from_slice(&1u32.to_le_bytes());
        data[80..88].copy_from_slice(&(u64::MAX - 10).to_le_bytes());
        data[104..112].copy_from_slice(&20u64.to_le_bytes());
        assert!(Elf::parse(&data).unwrap_err().contains("address overflow"));
        data[80..88].copy_from_slice(&0u64.to_le_bytes());
        data[104..112].copy_from_slice(&(LIMIT + 1).to_le_bytes());
        assert!(Elf::parse(&data).unwrap_err().contains("memory segment"));
    }
    #[test]
    fn truncated_elf_headers_are_rejected() {
        for length in 0..64 {
            let mut data = vec![0; length];
            if length >= 7 {
                data[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
            }
            assert!(Elf::parse(&data).is_err());
        }
    }
}
