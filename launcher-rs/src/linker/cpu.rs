//! Applies only the reviewed, embedded CPU plan; no arbitrary runtime instruction plans.
use super::*;
use serde_json::{json, Value};
fn digest(name: &str, file: &str, image: Option<&str>) -> String {
    let known = match name {
        "eboot.bin" => (
            "071df19c8880086d97182dbc057bc8cb37badaca57d9112683836b24a0444c0a",
            "d65f0b4f01d59166aed16f8604196d8b7dd805abbf0758b356e8f1354c9429f9",
        ),
        "libc.prx" => (
            "6ad54a1be85bdaaa7db7e3e9de4e659967aa983d808b4beee27fcdc9bf8459e5",
            "4378b47f46f1d856824a6f971db1a0c3f41833b77f49d2fba9538f667a139166",
        ),
        "libSceFios2.prx" => (
            "f8502e888b67112c873ee6e7b4700179b56c58efe4a6cc43c743fd22e9c9bedd",
            "3b8eb4d5a5c5a7b381d8d0610077e6cd404c29695a686274802f6b2737f6bf22",
        ),
        _ => ("", ""),
    };
    if image == Some(known.0) {
        known.1.to_string()
    } else {
        file.to_string()
    }
}
fn hex(value: &Value) -> Result<Vec<u8>> {
    let s = value.as_str().ok_or("CPU plan bytes are not a string")?;
    if !s.is_ascii() || s.len() % 2 != 0 {
        return Err("Invalid CPU plan hex".into());
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|e| e.to_string()))
        .collect()
}
fn number(value: &Value) -> Result<u64> {
    value.as_u64().ok_or("Invalid CPU plan integer".into())
}
fn relative(to: u64, next: u64) -> Result<[u8; 4]> {
    Ok(i32::try_from(to as i64 - next as i64)
        .map_err(|_| "CPU plan jump out of range")?
        .to_le_bytes())
}
pub(super) fn patch_libc(module: &mut Module) -> Result<Value> {
    if !legacy_cpu_enabled() {
        return Ok(json!([]));
    }
    if digest("libc.prx", &module.sha256, module.image_sha256.as_deref())
        != "4378b47f46f1d856824a6f971db1a0c3f41833b77f49d2fba9538f667a139166"
    {
        return Err("CPU compatibility libc source changed".into());
    }
    let nid = formats::nid("strlen")?;
    let matches: Vec<_> = module
        .symbols
        .iter()
        .filter(|s| s.section != 0 && s.kind == 2 && s.name.split('#').next() == Some(nid.as_str()))
        .collect();
    if matches.len() != 1 {
        return Err("CPU compatibility strlen is ambiguous".into());
    }
    let s = matches[0];
    let p = module
        .elf
        .segments
        .iter()
        .find(|p| {
            p.kind == 1
                && p.flags & 1 != 0
                && p.address <= s.value
                && s.value + s.size <= p.address + p.file_size
        })
        .ok_or("strlen is outside executable data")?;
    let offset = (p.offset + s.value - p.address) as usize;
    let original = formats::span(&module.elf.bytes, offset as u64, s.size)?.to_vec();
    if s.value != 0x2f970
        || s.size != 224
        || original.get(24..29) != Some(&[0xc4, 0xe2, 0xf8, 0xf2, 0xc6])
    {
        return Err("CPU compatibility strlen signature changed".into());
    }
    module.elf.bytes[offset..offset + 224].fill(0x90);
    let body = hex(&json!("31c0803c0700740548ffc0ebf5c3"))?;
    module.elf.bytes[offset..offset + body.len()].copy_from_slice(&body);
    Ok(
        json!([{"function":"strlen","address":"0x2f970","original_sha256":format!("{:x}",Sha256::digest(&original)),"replacement":"baseline x86-64, no BMI1"}]),
    )
}
pub(super) fn patch(
    image: &mut Vec<u8>,
    segments: &mut Vec<Load>,
    main: &Module,
    modules: &[LinkedModule],
    relocs: &[Fixup],
) -> Result<Value> {
    let active = legacy_cpu_enabled();
    let mut report = json!({"schema_version":1,"enabled":active,"source_hashes":{},"plan_sha256":null,"patched_sites":0,"counts_by_instruction":{},"trampoline_bytes":0,"unhandled_count":0,"unhandled_by_reason":{},"unhandled_by_instruction":{},"libc":[],"complete_coverage":false,"experimental":active,"semantic_risk_sites":0,"plan_version":null,"span_patches":0,"span_sites":[],"spans_experimental":false,"span_entry_safety":"not_applicable","span_entry_evidence":[],"span_execution_allowed":false});
    if !active {
        return Ok(report);
    }
    let raw = include_bytes!("../../../tools/legacy-cpu-plan.json");
    let plan: Value = serde_json::from_slice(raw).map_err(|e| e.to_string())?;
    let mut locations = BTreeMap::new();
    for m in modules {
        locations.insert(
            m.file.as_str(),
            (
                m.base,
                m.size,
                digest(&m.file, &m.sha256, m.image_sha256.as_deref()),
            ),
        );
    }
    let size = modules
        .iter()
        .map(|m| m.base)
        .min()
        .unwrap_or(image.len() as u64);
    locations.insert(
        "eboot.bin",
        (
            0,
            size,
            digest("eboot.bin", &main.sha256, main.image_sha256.as_deref()),
        ),
    );
    if plan["modules"]
        .as_object()
        .ok_or("CPU plan modules missing")?
        .len()
        != locations.len()
    {
        return Err("CPU plan module set changed".into());
    }
    for (name, (_, _, hash)) in &locations {
        if plan["modules"][*name].as_str() != Some(hash.as_str()) {
            return Err(format!("CPU plan source mismatch: {name}"));
        }
    }
    let mapped = |start: u64, size: u64, exec: bool| {
        segments.iter().any(|s| {
            s.address <= start
                && start
                    .checked_add(size)
                    .is_some_and(|e| e <= s.address + s.size)
                && (!exec || s.flags & 1 != 0)
        })
    };
    let arena = align(image.len() as u64, 65536)?;
    let mut cursor = arena;
    let mut operations = Vec::new();
    let mut counts = BTreeMap::<String, u64>::new();
    let mut spans = Vec::new();
    let mut evidence = Vec::new();
    for site in plan["sites"].as_array().ok_or("CPU plan sites missing")? {
        let name = site["module"].as_str().ok_or("CPU module missing")?;
        let (base, size, _) = locations.get(name).ok_or("Unknown CPU module")?;
        let address = number(&site["address"])?;
        let original = hex(&site["bytes"])?;
        let mut body = hex(&site["body"])?;
        let start = base.checked_add(address).ok_or("CPU address overflow")?;
        let end = start + original.len() as u64;
        if original.len() < 5
            || address + original.len() as u64 > *size
            || !mapped(start, original.len() as u64, true)
            || image.get(start as usize..end as usize) != Some(original.as_slice())
        {
            return Err(format!(
                "CPU instruction signature changed at {name}+{address:#x}"
            ));
        }
        if relocs
            .iter()
            .any(|r| r.target >= start.saturating_sub(7) && r.target < end)
        {
            return Err("CPU instruction overlaps a relocation".into());
        }
        let kind = site["kind"].as_str().unwrap_or("instruction");
        let is_span = kind != "instruction";
        if is_span {
            let interior = |target: i128| (start as i128) < target && target < end as i128;
            if relocs
                .iter()
                .any(|r| r.kind == 0 && interior(r.value as i128 + r.addend as i128))
                || (name == "eboot.bin"
                    && main
                        .symbols
                        .iter()
                        .any(|s| s.section != 0 && interior((base + s.value) as i128)))
            {
                return Err("CPU span has an interior entry".into());
            }
            if kind == "count_span" {
                let span = &site["span"];
                let fs = number(&span["function_start"])?;
                let fe = number(&span["function_end"])?;
                if fs > address
                    || fe < address + original.len() as u64
                    || fe < fs
                    || !mapped(fs, fe - fs, true)
                    || format!("{:x}", Sha256::digest(formats::span(image, fs, fe - fs)?))
                        != span["function_sha256"]
                            .as_str()
                            .ok_or("Missing CPU function hash")?
                {
                    return Err("CPU count span function changed".into());
                }
                for r in &main.relocations {
                    let target = if r.kind == 8 {
                        Some(r.addend as i128)
                    } else if [1, 6, 7].contains(&r.kind) {
                        main.symbols
                            .get(r.symbol)
                            .filter(|s| s.section != 0)
                            .map(|s| s.value as i128 + r.addend as i128)
                    } else {
                        None
                    };
                    if target.is_some_and(interior) {
                        return Err("CPU count span has an ELF interior entry".into());
                    }
                }
                body.extend_from_slice(&original[4..]);
                let e = &span["entry_evidence"];
                let e = json!({"reference":e["reference"],"sha256":e["sha256"],"method":e["method"],"global_indirect_proof":e["global_indirect_proof"]});
                if !evidence.contains(&e) {
                    evidence.push(e);
                }
            } else if kind != "short_span" {
                return Err("Unknown embedded CPU site kind".into());
            }
        }
        cursor = align(cursor, 16)?;
        let mut fixed = std::collections::BTreeSet::new();
        for f in site["rip_fixups"]
            .as_array()
            .ok_or("Missing CPU RIP fixups")?
        {
            let off = number(&f["offset"])? as usize;
            let next = number(&f["next_offset"])?;
            let target = number(&f["target"])?;
            if off + 4 > body.len()
                || off as u64 + 4 > next
                || next > body.len() as u64
                || target >= *size
                || !mapped(base + target, 1, false)
                || (off..off + 4).any(|i| !fixed.insert(i))
            {
                return Err("Invalid CPU RIP fixup".into());
            }
            body[off..off + 4].copy_from_slice(&relative(base + target, cursor + next)?);
        }
        let mut jump = vec![0xe9];
        jump.extend_from_slice(&relative(cursor, start + 5)?);
        jump.resize(original.len(), 0x90);
        let back = relative(end, cursor + body.len() as u64 + 5)?;
        body.push(0xe9);
        body.extend_from_slice(&back);
        if is_span {
            spans.push(json!({"module":name,"address":address,"image_start":start,"image_end":end,"replacement_bytes":jump.iter().map(|b|format!("{b:02x}")).collect::<String>(),"trampoline_start":cursor,"trampoline_size":body.len(),"trampoline_sha256":format!("{:x}",Sha256::digest(&body))}));
        }
        let next = cursor + body.len() as u64;
        operations.push((start, end, cursor, body, jump));
        cursor = next;
        if cursor > LIMIT {
            return Err("CPU image exceeds limit".into());
        }
        let instruction = site["instruction"]
            .as_str()
            .ok_or("Missing CPU instruction")?
            .split(' ')
            .next()
            .unwrap()
            .to_string();
        *counts.entry(instruction).or_default() += 1;
    }
    let mut ranges: Vec<_> = operations.iter().map(|x| (x.0, x.1)).collect();
    ranges.sort_unstable();
    if ranges.windows(2).any(|r| r[0].1 > r[1].0) {
        return Err("CPU sites overlap".into());
    }
    let partial =
        std::env::var("BB_LEGACY_CPU_ALLOW_PARTIAL").unwrap_or_else(|_| "1".into()) == "1";
    if !partial && !spans.is_empty() {
        return Err("Experimental CPU spans require BB_LEGACY_CPU_ALLOW_PARTIAL=1".into());
    }
    let unhandled = plan["unhandled"].as_array().ok_or("Missing CPU coverage")?;
    // The embedded reviewed plan currently has no unhandled sites. Reject changes
    // until their coverage and policy are deliberately ported too.
    if !unhandled.is_empty() {
        return Err("Embedded CPU plan has unhandled instructions".into());
    }
    let end = align(cursor, 4096)?;
    if end > LIMIT {
        return Err("CPU aligned image exceeds limit".into());
    }
    if !operations.is_empty() {
        image.resize(end as usize, 0);
        for (start, end, target, body, jump) in &operations {
            image[*start as usize..*end as usize].copy_from_slice(jump);
            image[*target as usize..*target as usize + body.len()].copy_from_slice(body);
        }
        segments.push(Load {
            address: arena,
            size: end - arena,
            flags: 5,
        });
    }
    report["source_hashes"] = locations
        .iter()
        .map(|(name, (_, _, h))| ((*name).to_string(), json!(h)))
        .collect::<serde_json::Map<_, _>>()
        .into();
    report["plan_sha256"] = format!("{:x}", Sha256::digest(raw)).into();
    report["plan_version"] = plan["version"].clone();
    report["patched_sites"] = operations.len().into();
    report["counts_by_instruction"] = json!(counts);
    report["trampoline_bytes"] = (cursor - arena).into();
    report["coverage"] = plan["coverage"].clone();
    report["partial_execution_allowed"] = partial.into();
    report["span_patches"] = spans.len().into();
    report["spans_experimental"] = (!spans.is_empty()).into();
    report["span_execution_allowed"] = (!spans.is_empty() && partial).into();
    report["span_entry_safety"] = if spans.is_empty() {
        "not_applicable"
    } else {
        "known_entries_checked_global_indirect_unproven"
    }
    .into();
    report["span_sites"] = spans.into();
    report["span_entry_evidence"] = evidence.into();
    Ok(report)
}
