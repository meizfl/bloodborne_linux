//! Native probe-image preparation. Game files are read only; outputs are atomic.
mod cpu;
use crate::config::atomic_write;
use crate::formats::{self, Elf, Segment};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::Path;
type Result<T> = std::result::Result<T, String>;
const LIMIT: u64 = 512 * 1024 * 1024;
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct Versioned {
    name: String,
    version: u16,
}
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct Identity {
    nid: String,
    library: Versioned,
    module: Versioned,
}
#[derive(Clone, Debug)]
struct Symbol {
    name: String,
    kind: u8,
    binding: u8,
    section: u16,
    value: u64,
    size: u64,
    identity: Option<Identity>,
}
#[derive(Clone, Debug)]
struct Relocation {
    target: u64,
    kind: u32,
    symbol: usize,
    addend: i64,
}
struct Module {
    elf: Elf,
    tags: BTreeMap<u64, u64>,
    symbols: Vec<Symbol>,
    relocations: Vec<Relocation>,
    libraries: BTreeMap<String, Versioned>,
    modules: BTreeMap<String, Versioned>,
    sha256: String,
    image_sha256: Option<String>,
}
fn encode_id(mut id: u64) -> String {
    let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+-";
    let mut result = vec![alphabet[(id & 63) as usize]];
    id >>= 6;
    while id != 0 {
        result.push(alphabet[(id & 63) as usize]);
        id >>= 6;
    }
    result.reverse();
    String::from_utf8(result).unwrap()
}
impl Module {
    fn load(path: &Path) -> Result<Self> {
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let sha256 = format!("{:x}", Sha256::digest(&bytes));
        let elf = Elf::parse(&bytes)?;
        let image_sha256 = elf.image_hash()?;
        let mut libraries = BTreeMap::new();
        let mut modules = BTreeMap::new();
        let mut tags = BTreeMap::new();
        let mut dynamic = Vec::new();
        let part = elf.segment(2)?;
        for at in (part.offset..part.offset + part.file_size).step_by(16) {
            let tag = formats::u64_at(&elf.bytes, at)?;
            if tag == 0 {
                break;
            }
            let value = formats::u64_at(&elf.bytes, at + 8)?;
            tags.insert(tag, value);
            dynamic.push((tag, value));
        }
        let strings = elf.table(0x61000035, 0x61000037)?;
        let string = |offset: u64| -> Result<String> {
            let text = formats::c_string(strings, offset)?;
            if !text.is_ascii() {
                return Err("Non-ASCII native symbol".into());
            }
            Ok(text.into())
        };
        for (tag, value) in &dynamic {
            if matches!(*tag, 0x61000013 | 0x61000015 | 0x6100000d | 0x6100000f) {
                let table = if matches!(*tag, 0x61000013 | 0x61000015) {
                    &mut libraries
                } else {
                    &mut modules
                };
                let key = encode_id(value >> 48);
                let identity = Versioned {
                    name: string(value & 0xffffffff)?,
                    version: ((value >> 32) & 65535) as u16,
                };
                if let Some(old) = table.insert(key, identity.clone()) {
                    if old != identity {
                        return Err("Conflicting library/module IDs".into());
                    }
                }
            }
        }
        let table = elf.table(0x61000039, 0x6100003f)?;
        if table.len() % 24 != 0 {
            return Err("Invalid symbol table size".into());
        }
        let mut symbols = Vec::new();
        for at in (0..table.len() as u64).step_by(24) {
            let name = string(u64::from(formats::u32_at(table, at)?))?;
            let info = formats::span(table, at + 4, 1)?[0];
            let parts: Vec<_> = name.split('#').collect();
            let identity = if parts.len() == 3 {
                Some(Identity {
                    nid: parts[0].into(),
                    library: libraries.get(parts[1]).ok_or("Unknown library ID")?.clone(),
                    module: modules.get(parts[2]).ok_or("Unknown module ID")?.clone(),
                })
            } else {
                None
            };
            symbols.push(Symbol {
                name,
                kind: info & 15,
                binding: info >> 4,
                section: formats::u16_at(table, at + 6)?,
                value: formats::u64_at(table, at + 8)?,
                size: formats::u64_at(table, at + 16)?,
                identity,
            });
        }
        let mut relocations = Vec::new();
        for (offset, size) in [(0x61000029, 0x6100002d), (0x6100002f, 0x61000031)] {
            let table = elf.table(offset, size)?;
            if table.len() % 24 != 0 {
                return Err("Invalid relocation table size".into());
            }
            for at in (0..table.len() as u64).step_by(24) {
                let info = formats::u64_at(table, at + 8)?;
                relocations.push(Relocation {
                    target: formats::u64_at(table, at)?,
                    kind: info as u32,
                    symbol: (info >> 32) as usize,
                    addend: formats::i64_at(table, at + 16)?,
                });
            }
        }
        Ok(Self {
            elf,
            tags,
            symbols,
            relocations,
            libraries,
            modules,
            sha256,
            image_sha256,
        })
    }
    fn symbol(&self, index: usize) -> Result<&Symbol> {
        self.symbols
            .get(index)
            .ok_or("Relocation references missing symbol".into())
    }
    fn init_env_is_ret(&self) -> Result<bool> {
        let nid = formats::nid("_init_env")?;
        for symbol in &self.symbols {
            if symbol.name.split('#').next() != Some(&nid)
                || symbol.section == 0
                || symbol.kind != 2
            {
                continue;
            }
            if let Some(segment) = self.elf.segments.iter().find(|p| {
                p.kind == 1
                    && p.flags & 1 != 0
                    && symbol.value >= p.address
                    && symbol.value < p.address + p.file_size
            }) {
                return Ok(formats::span(
                    &self.elf.bytes,
                    segment.offset + symbol.value - segment.address,
                    symbol.size,
                )? == [0xc3]);
            }
        }
        Ok(false)
    }
}
#[derive(Clone, Debug)]
struct Load {
    address: u64,
    size: u64,
    flags: u64,
}
#[derive(Clone, Debug)]
struct Fixup {
    target: u64,
    kind: u64,
    value: i64,
    addend: i64,
}
struct LinkedModule {
    file: String,
    base: u64,
    size: u64,
    init: u64,
    tls_address: u64,
    tls_memory: u64,
    tls_file: u64,
    tls_module: u64,
    tls_relocations: usize,
    exports: usize,
    sha256: String,
    image_sha256: Option<String>,
}
fn align(value: u64, alignment: u64) -> Result<u64> {
    value
        .checked_add(alignment - 1)
        .map(|v| v & !(alignment - 1))
        .ok_or("Alignment overflow".into())
}
fn address(base: u64, value: u64, addend: i64) -> Result<u64> {
    base.checked_add(value)
        .and_then(|v| v.checked_add_signed(addend))
        .ok_or("Relocation address overflow".into())
}
fn write_u64(image: &mut [u8], at: u64, value: u64) -> Result<()> {
    formats::span(image, at, 8)?;
    image[at as usize..at as usize + 8].copy_from_slice(&value.to_le_bytes());
    Ok(())
}
fn signed(value: u64) -> Result<i64> {
    i64::try_from(value).map_err(|_| "Relocation exceeds signed address range".into())
}
fn fs_to_gs(image: &mut [u8], segments: &[Segment], base: u64) -> Result<usize> {
    let pattern = b"\x64\x48\x8b\x04\x25\x00\x00\x00\x00";
    let mut patched = 0;
    for part in segments.iter().filter(|p| p.kind == 1 && p.flags & 1 != 0) {
        let start = base
            .checked_add(part.address)
            .ok_or("TLS address overflow")?;
        formats::span(image, start, part.file_size)?;
        let end = (start + part.file_size) as usize;
        let mut pos = start as usize;
        while pos + pattern.len() <= end {
            if &image[pos..pos + pattern.len()] == pattern {
                image[pos] = 0x65;
                patched += 1;
                pos += pattern.len();
            } else {
                pos += 1;
            }
        }
    }
    Ok(patched)
}
fn push_u64(output: &mut Vec<u8>, value: u64) {
    output.extend_from_slice(&value.to_le_bytes());
}
fn write_imports(output: &mut Vec<u8>, names: &[String]) -> Result<()> {
    for name in names {
        if name.len() >= 128 {
            return Err("Native import name is too long".into());
        }
        output.extend_from_slice(name.as_bytes());
        output.resize(output.len() + 128 - name.len(), 0);
    }
    Ok(())
}
fn write_segments(output: &mut Vec<u8>, segments: &[Load]) {
    for part in segments {
        for value in [part.address, part.size, part.flags] {
            push_u64(output, value);
        }
    }
}
fn write_fixups(output: &mut Vec<u8>, fixups: &[Fixup]) {
    for r in fixups {
        push_u64(output, r.target);
        push_u64(output, r.kind);
        output.extend_from_slice(&r.value.to_le_bytes());
        output.extend_from_slice(&r.addend.to_le_bytes());
    }
}
fn header(
    output: &mut Vec<u8>,
    magic: &[u8; 8],
    size: u64,
    entry: u64,
    segments: usize,
    fixups: usize,
    names: usize,
    flags: u64,
) {
    output.extend_from_slice(magic);
    for value in [
        size,
        entry,
        segments as u64,
        fixups as u64,
        names as u64,
        flags,
    ] {
        push_u64(output, value);
    }
}

pub fn prepare(game: &Path, out: &Path) -> Result<serde_json::Value> {
    let main = Module::load(&game.join("eboot.bin"))?;
    let initial = main.elf.image()?;
    let initial_size = initial.len() as u64;
    let mut image = initial.clone();
    let mut segments: Vec<Load> = main
        .elf
        .segments
        .iter()
        .filter(|p| p.loadable())
        .map(|p| Load {
            address: p.address,
            size: p.memory_size,
            flags: u64::from(p.flags),
        })
        .collect();
    let mut names = Vec::new();
    let mut imported_main = BTreeMap::new();
    let mut fixups = Vec::new();
    for relocation in &main.relocations {
        if !main.elf.mapped(relocation.target, 8) {
            return Err("Main relocation is outside mapped segments".into());
        }
        match relocation.kind {
            8 => fixups.push(Fixup {
                target: relocation.target,
                kind: 0,
                value: relocation.addend,
                addend: 0,
            }),
            1 | 6 | 7 => {
                let symbol = main.symbol(relocation.symbol)?;
                if symbol.section != 0 {
                    let value = address(0, symbol.value, relocation.addend)?;
                    if symbol.section == 0xfff1 {
                        write_u64(&mut image, relocation.target, value)?;
                    } else {
                        fixups.push(Fixup {
                            target: relocation.target,
                            kind: 0,
                            value: signed(value)?,
                            addend: 0,
                        });
                    }
                } else {
                    let next = names.len();
                    let index = *imported_main.entry(relocation.symbol).or_insert_with(|| {
                        names.push(symbol.name.clone());
                        next
                    });
                    let kind = if symbol.kind == 1 { 2 } else { 1 };
                    if (kind == 1 && relocation.addend != 0)
                        || !(0..4096).contains(&relocation.addend)
                    {
                        return Err("Unsupported native import addend".into());
                    }
                    fixups.push(Fixup {
                        target: relocation.target,
                        kind,
                        value: index as i64,
                        addend: relocation.addend,
                    });
                }
            }
            other => return Err(format!("Unsupported main relocation {other}")),
        }
    }
    let mut libc = Module::load(&game.join("sce_module/libc.prx"))?;
    let libc_changes = cpu::patch_libc(&mut libc)?;
    let flags = u64::from(libc.init_env_is_ret()?);
    let mut boot2 = Vec::new();
    header(
        &mut boot2,
        b"BBPROBE2",
        initial_size,
        main.elf.header.entry,
        segments.len(),
        fixups.len(),
        names.len(),
        flags,
    );
    write_segments(&mut boot2, &segments);
    write_imports(&mut boot2, &names)?;
    write_fixups(&mut boot2, &fixups);
    boot2.extend_from_slice(&image);
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    atomic_write(&out.join("boot.bin"), &boot2).map_err(|e| e.to_string())?;
    drop(boot2);
    atomic_write(&out.join("eboot.elf"), &main.elf.bytes).map_err(|e| e.to_string())?;
    atomic_write(&out.join("entry.bin"), &image[..image.len().min(1024)])
        .map_err(|e| e.to_string())?;
    let mut identities: Vec<(Option<Identity>, usize)> = Vec::new();
    let mut positions: BTreeMap<Option<Identity>, usize> = BTreeMap::new();
    for (index, name) in names.iter().enumerate() {
        let identity = main
            .symbols
            .iter()
            .find(|s| s.name == *name)
            .ok_or("Missing imported symbol")?
            .identity
            .clone();
        if let Some(pos) = positions.get(&identity) {
            identities[*pos].1 = index;
        } else {
            positions.insert(identity.clone(), identities.len());
            identities.push((identity, index));
        }
    }
    let main_libraries: BTreeMap<Versioned, String> = main
        .libraries
        .iter()
        .map(|(key, value)| (value.clone(), key.clone()))
        .collect();
    let main_modules: BTreeMap<Versioned, String> = main
        .modules
        .iter()
        .map(|(key, value)| (value.clone(), key.clone()))
        .collect();
    let mut exports: BTreeMap<Identity, (u64, u64)> = BTreeMap::new();
    let mut by_nid: BTreeMap<(String, String), Vec<(u64, u64)>> = BTreeMap::new();
    let mut table = Vec::new();
    let mut base = align(initial_size, 65536)?;
    let mut tls_module = 2u64;
    let mut fs_patched = fs_to_gs(&mut image, &main.elf.segments, 0)?;
    for (filename, module) in [
        ("libc.prx", libc),
        (
            "libSceFios2.prx",
            Module::load(&game.join("sce_module/libSceFios2.prx"))?,
        ),
    ] {
        let module_image = module.elf.image()?;
        let size = module_image.len() as u64;
        if base.checked_add(size).ok_or("Linked image overflow")? > LIMIT {
            return Err("Linked image exceeds probe limit".into());
        }
        image.resize((base + size) as usize, 0);
        image[base as usize..(base + size) as usize].copy_from_slice(&module_image);
        for p in module.elf.segments.iter().filter(|p| p.loadable()) {
            segments.push(Load {
                address: base + p.address,
                size: p.memory_size,
                flags: u64::from(p.flags),
            });
        }
        let tls = module.elf.segments.iter().find(|p| p.kind == 7);
        let module_id = if let Some(tls) = tls {
            if tls.file_size > tls.memory_size || !module.elf.mapped(tls.address, tls.memory_size) {
                return Err(format!("{filename}: unsupported TLS layout"));
            }
            let id = tls_module;
            tls_module += 1;
            id
        } else {
            0
        };
        let mut tls_relocations = 0;
        for relocation in &module.relocations {
            if !module.elf.mapped(relocation.target, 8) {
                return Err(format!("{filename}: relocation outside image"));
            }
            let target = base + relocation.target;
            match relocation.kind {
                8 => fixups.push(Fixup {
                    target,
                    kind: 0,
                    value: signed(address(base, 0, relocation.addend)?)?,
                    addend: 0,
                }),
                16 => {
                    let symbol = module.symbol(relocation.symbol)?;
                    if !symbol.name.is_empty()
                        || symbol.kind != 3
                        || relocation.addend != 0
                        || module_id == 0
                    {
                        return Err(format!("{filename}: unsupported external TLS relocation"));
                    }
                    write_u64(&mut image, target, module_id)?;
                    tls_relocations += 1;
                }
                1 | 6 | 7 => {
                    let symbol = module.symbol(relocation.symbol)?;
                    if symbol.section != 0 {
                        if symbol.section == 0xfff1 {
                            write_u64(
                                &mut image,
                                target,
                                address(0, symbol.value, relocation.addend)?,
                            )?;
                        } else {
                            fixups.push(Fixup {
                                target,
                                kind: 0,
                                value: signed(address(base, symbol.value, relocation.addend)?)?,
                                addend: 0,
                            });
                        }
                    } else if symbol.identity.is_none() && symbol.binding == 2 {
                        write_u64(&mut image, target, 0)?;
                    } else {
                        let identity = symbol
                            .identity
                            .clone()
                            .ok_or("Unlinkable strong native import")?;
                        let kind = if symbol.kind == 1 { 2 } else { 1 };
                        if (kind == 1 && relocation.addend != 0)
                            || !(0..4096).contains(&relocation.addend)
                        {
                            return Err(format!("{filename}: unsupported import addend"));
                        }
                        let key = Some(identity.clone());
                        let index = if let Some(pos) = positions.get(&key) {
                            identities[*pos].1
                        } else {
                            let name = if let (Some(lib), Some(module)) = (
                                main_libraries.get(&identity.library),
                                main_modules.get(&identity.module),
                            ) {
                                format!("{}#{lib}#{module}", identity.nid)
                            } else {
                                format!("{}#{}", identity.nid, identity.library.name)
                            };
                            let index = names.len();
                            names.push(name);
                            positions.insert(key.clone(), identities.len());
                            identities.push((key, index));
                            index
                        };
                        fixups.push(Fixup {
                            target,
                            kind,
                            value: index as i64,
                            addend: relocation.addend,
                        });
                    }
                }
                other => return Err(format!("{filename}: unsupported relocation {other}")),
            }
        }
        let mut count = 0;
        for symbol in &module.symbols {
            if symbol.section == 0
                || !matches!(symbol.binding, 1 | 2)
                || !matches!(symbol.kind, 1 | 2)
            {
                continue;
            }
            if let Some(identity) = &symbol.identity {
                if !module.elf.mapped(symbol.value, symbol.size.max(1)) {
                    return Err(format!("{filename}: export outside load segments"));
                }
                let value = (base + symbol.value, if symbol.kind == 1 { 2 } else { 1 });
                if exports.insert(identity.clone(), value).is_some() {
                    return Err(format!("{filename}: ambiguous export"));
                }
                by_nid
                    .entry((identity.nid.clone(), identity.library.name.clone()))
                    .or_default()
                    .push(value);
                count += 1;
            }
        }
        fs_patched += fs_to_gs(&mut image, &module.elf.segments, base)?;
        table.push(LinkedModule {
            file: filename.into(),
            base,
            size,
            init: base + module.tags.get(&12).copied().unwrap_or(0),
            tls_address: tls.map_or(0, |p| base + p.address),
            tls_memory: tls.map_or(0, |p| p.memory_size),
            tls_file: tls.map_or(0, |p| p.file_size),
            tls_module: module_id,
            tls_relocations,
            exports: count,
            sha256: module.sha256,
            image_sha256: module.image_sha256,
        });
        base = align(base + size, 65536)?;
    }
    let mut bindings = Vec::new();
    let mut unresolved = Vec::new();
    for (identity, index) in &identities {
        let mut found = identity.as_ref().and_then(|id| exports.get(id)).copied();
        if found.is_none() {
            if let Some(identity) = identity {
                if identity.library.name == "libSceLibcInternal" {
                    if let Some(candidates) = by_nid.get(&(identity.nid.clone(), "libc".into())) {
                        if candidates.len() == 1 {
                            found = Some(candidates[0]);
                        }
                    }
                }
            }
        }
        if let Some((value, kind)) = found {
            bindings.push((*index as u64, value, kind));
        } else {
            unresolved.push(names[*index].clone());
        }
    }
    let tls = main.elf.segments.iter().find(|p| p.kind == 7);
    if let Some(p) = tls {
        if p.file_size > p.memory_size
            || p.memory_size > 1024 * 1024
            || p.address + p.file_size > initial_size
        {
            return Err("Unsupported eboot TLS layout".into());
        }
    }
    let tls_values = tls.map_or([0; 4], |p| {
        [p.address, p.file_size, p.memory_size, p.alignment]
    });
    let procparam = main.elf.segment(0x61000001)?.address;
    let untranslated = image.clone();
    let mut cpu_compat = cpu::patch(&mut image, &mut segments, &main, &table, &fixups)?;
    cpu_compat["libc"] = libc_changes;
    let red_zone = protect_red_zones(
        &mut image,
        &mut segments,
        &fixups,
        &untranslated,
        red_zone_enabled(),
    )?;
    let mut output = Vec::with_capacity(image.len() + 1024 * 1024);
    header(
        &mut output,
        b"BBPROBE5",
        image.len() as u64,
        main.elf.header.entry,
        segments.len(),
        fixups.len(),
        names.len(),
        flags,
    );
    push_u64(&mut output, procparam);
    for value in tls_values {
        push_u64(&mut output, value);
    }
    push_u64(&mut output, table.len() as u64);
    for module in &table {
        for value in [
            module.base,
            module.size,
            module.init,
            module.tls_address,
            module.tls_memory,
            module.tls_file,
            module.tls_module,
        ] {
            push_u64(&mut output, value);
        }
    }
    push_u64(&mut output, bindings.len() as u64);
    for (index, value, kind) in &bindings {
        for number in [*index, *value, *kind] {
            push_u64(&mut output, number);
        }
    }
    write_segments(&mut output, &segments);
    write_imports(&mut output, &names)?;
    write_fixups(&mut output, &fixups);
    output.extend_from_slice(&image);
    atomic_write(&out.join("boot-linked.bin"), &output).map_err(|e| e.to_string())?;
    let report = serde_json::json!({"modules":table.iter().map(|m|serde_json::json!({"file":m.file,"base":format!("{:#x}",m.base),"size":m.size,"init":format!("{:#x}",m.init),"tls_address":format!("{:#x}",m.tls_address),"tls_memsz":m.tls_memory,"tls_filesz":m.tls_file,"tls_module":m.tls_module,"tls_relocations":m.tls_relocations,"exports":m.exports,"sha256":m.sha256,"image_sha256":m.image_sha256})).collect::<Vec<_>>(),"bindings":bindings.len(),"imports":names.len(),"fs_loads_patched":fs_patched,"unresolved_imports":unresolved,"main_tls":{"vaddr":tls_values[0],"filesz":tls_values[1],"memsz":tls_values[2],"align":tls_values[3]},"cpu_compat":cpu_compat,"red_zone":red_zone});
    atomic_write(
        &out.join("link.json"),
        serde_json::to_string_pretty(&report)
            .map_err(|e| e.to_string())?
            .as_bytes(),
    )
    .map_err(|e| e.to_string())?;
    let summary = serde_json::json!({"source_sha256":main.sha256,"image_sha256":main.image_sha256,"entry":format!("{:#x}",main.elf.header.entry),"image_bytes":initial_size,"import_count":names.len()});
    atomic_write(
        &out.join("analysis.json"),
        serde_json::to_string_pretty(&summary)
            .map_err(|e| e.to_string())?
            .as_bytes(),
    )
    .map_err(|e| e.to_string())?;
    Ok(report)
}
fn legacy_cpu_enabled() -> bool {
    if let Ok(value) = std::env::var("BB_LEGACY_CPU") {
        return value == "1";
    }
    let config = std::env::var_os("BB_CONFIG")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| game_config_default());
    if let Ok(text) = std::fs::read_to_string(config) {
        for line in text.lines().map(str::trim) {
            if line == "legacy_cpu=0" || line == "legacy_cpu=1" {
                return line == "legacy_cpu=1";
            }
        }
    }
    #[cfg(target_arch = "x86_64")]
    unsafe {
        use std::arch::x86_64::__cpuid_count;
        let top = __cpuid_count(0, 0).eax;
        let extended = __cpuid_count(0x80000000, 0).eax;
        return top < 7
            || __cpuid_count(7, 0).ebx & (1 << 3) == 0
            || __cpuid_count(1, 0).ecx & (1 << 22) == 0
            || extended < 0x80000001
            || __cpuid_count(0x80000001, 0).ecx & (1 << 5) == 0;
    }
    #[cfg(not(target_arch = "x86_64"))]
    true
}
fn game_config_default() -> std::path::PathBuf {
    crate::paths::Paths::detect()
        .map(|p| p.ini())
        .unwrap_or_else(|_| "bbport.ini".into())
}
fn red_zone_enabled() -> bool {
    std::env::var("BB_RED_ZONE")
        .map(|value| value == "1")
        .unwrap_or(cfg!(windows))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nearby_red_zone_sites_do_not_become_cpu_overlaps() {
        let plan: serde_json::Value =
            serde_json::from_slice(include_bytes!("../../patches/redzone.json")).unwrap();
        let sites = plan["sites"].as_array().unwrap();
        let size = sites
            .iter()
            .map(|s| s[0].as_u64().unwrap() as usize + s[1].as_str().unwrap().len() / 2)
            .max()
            .unwrap();
        let mut image = vec![0; size];
        for site in sites {
            let offset = site[0].as_u64().unwrap() as usize;
            let hex = site[1].as_str().unwrap();
            for i in (0..hex.len()).step_by(2) {
                image[offset + i / 2] = u8::from_str_radix(&hex[i..i + 2], 16).unwrap();
            }
        }
        let original = image.clone();
        let mut segments = Vec::new();
        let report = protect_red_zones(&mut image, &mut segments, &[], &original, true).unwrap();
        assert_eq!(report["sites"].as_u64(), Some(sites.len() as u64));
        assert_eq!(report["cpu_overlaps"], serde_json::json!([]));
        assert_eq!(segments.len(), 1);
    }
    #[test]
    fn encoded_ids_match_the_native_alphabet() {
        assert_eq!(encode_id(0), "A");
        assert_eq!(encode_id(63), "-");
        assert_eq!(encode_id(64), "BA");
    }
    #[test]
    fn relative_addresses_reject_underflow() {
        assert!(address(0, 1, -2).is_err());
        assert_eq!(address(64, 32, -1).unwrap(), 95);
    }
}

// Embedded upstream instruction plan; game data is never modified in place.
fn protect_red_zones(
    image: &mut Vec<u8>,
    segments: &mut Vec<Load>,
    fixups: &[Fixup],
    untranslated: &[u8],
    enabled: bool,
) -> Result<serde_json::Value> {
    let raw = include_bytes!("../../patches/redzone.json");
    let mut report = serde_json::json!({"enabled":enabled,"sites":0,"available":0,"sites_sha256":null,"cpu_overlaps":[],"trampoline_bytes":0,"segment":null});
    if !enabled {
        return Ok(report);
    }
    let plan: serde_json::Value = serde_json::from_slice(raw).map_err(|e| e.to_string())?;
    let sites = plan["sites"].as_array().ok_or("Invalid red-zone plan")?;
    let base = align(image.len() as u64, 65536)?;
    let mut code = Vec::new();
    let mut applied = 0;
    // Detect CPU overlaps against one immutable pre-red-zone image. Nearby
    // red-zone sites must not be mistaken for CPU translations of each other.
    let before_red_zone = image.clone();
    for site in sites {
        let offset = site[0].as_u64().ok_or("Invalid red-zone offset")?;
        let hex = site[1].as_str().ok_or("Invalid red-zone bytes")?;
        if hex.len() % 2 != 0 || !hex.is_ascii() {
            return Err("Invalid red-zone hex".into());
        }
        let original: Vec<u8> = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).map_err(|e| e.to_string()))
            .collect::<Result<_>>()?;
        let end = offset
            .checked_add(original.len() as u64)
            .ok_or("Red-zone address overflow")?;
        if before_red_zone.get(offset.saturating_sub(14) as usize..end as usize)
            != untranslated.get(offset.saturating_sub(14) as usize..end as usize)
        {
            report["cpu_overlaps"]
                .as_array_mut()
                .unwrap()
                .push(format!("{offset:#x}").into());
            continue;
        }
        if image.get(offset as usize..end as usize) != Some(original.as_slice())
            || fixups
                .iter()
                .any(|r| r.target > offset.saturating_sub(8) && r.target < end)
        {
            continue;
        }
        if original.len() < 5 {
            return Err("Red-zone jump site too short".into());
        }
        let lengths = site[2].as_array().ok_or("Invalid red-zone lengths")?;
        let here = base + code.len() as u64;
        let mut pos = offset as usize;
        for length in lengths {
            let length = length
                .as_i64()
                .ok_or("Invalid red-zone instruction length")?;
            let size = length.unsigned_abs() as usize;
            if size == 0
                || size > 15
                || pos
                    .checked_add(size)
                    .filter(|x| *x <= end as usize)
                    .is_none()
            {
                return Err("Invalid red-zone instruction boundary".into());
            }
            if length < 0 {
                code.extend_from_slice(&[0x48, 0x8d, 0x64, 0x24, 0x80]);
            }
            code.extend_from_slice(&image[pos..pos + size]);
            if length < 0 {
                code.extend_from_slice(&[0x48, 0x8d, 0xa4, 0x24, 0x80, 0, 0, 0]);
            }
            pos += size;
        }
        if pos != end as usize {
            return Err("Incomplete red-zone instruction plan".into());
        }
        let back = i32::try_from(end as i64 - (base + code.len() as u64 + 5) as i64)
            .map_err(|_| "Red-zone return outside rel32 range")?;
        code.push(0xe9);
        code.extend_from_slice(&back.to_le_bytes());
        while code.len() % 16 != 0 {
            code.push(0xcc);
        }
        let jump = i32::try_from(here as i64 - (offset + 5) as i64)
            .map_err(|_| "Red-zone jump outside rel32 range")?;
        image[offset as usize] = 0xe9;
        image[offset as usize + 1..offset as usize + 5].copy_from_slice(&jump.to_le_bytes());
        image[offset as usize + 5..end as usize].fill(0xcc);
        applied += 1;
    }
    if !code.is_empty() {
        let size = align(code.len() as u64, 4096)?;
        if base + size > LIMIT {
            return Err("Red-zone image exceeds probe limit".into());
        }
        image.resize(base as usize, 0);
        image.extend_from_slice(&code);
        image.resize((base + size) as usize, 0xcc);
        segments.push(Load {
            address: base,
            size,
            flags: 5,
        });
        report["segment"] = format!("{base:#x}").into();
    }
    report["sites"] = applied.into();
    report["available"] = sites.len().into();
    report["sites_sha256"] = format!("{:x}", Sha256::digest(raw)).into();
    report["trampoline_bytes"] = code.len().into();
    Ok(report)
}
