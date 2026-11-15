//! Shared render/output/HUD calculations, matching the existing runtime.
use super::*;
pub const NATIVE: (u32, u32) = (1920, 1080);
const SCALES: [f64; 5] = [1.0, 1.5, 1.7, 2.0, 3.0];
fn size(value: &str) -> Option<(u32, u32)> {
    let value = value.to_lowercase();
    let (w, h) = value.split_once('x')?;
    let (w, h) = (w.parse().ok()?, h.parse().ok()?);
    if w > 0 && h > 0 {
        Some((w, h))
    } else {
        None
    }
}
pub fn parse_size(value: &str) -> Option<(u32, u32)> {
    let value = value.trim().replace(['×', '*', 'X'], "x");
    let (w, h) = value.split_once('x')?;
    if !(3..=5).contains(&w.trim().len()) || !(3..=5).contains(&h.trim().len()) {
        return None;
    }
    let w = w.trim().parse::<u32>().ok()? / 2 * 2;
    let h = h.trim().parse::<u32>().ok()? / 2 * 2;
    if (640..=7680).contains(&w) && (360..=4320).contains(&h) {
        Some((w, h))
    } else {
        None
    }
}
fn scale(settings: &BTreeMap<String, String>) -> Result<f64> {
    if settings.get("upscaler").map(String::as_str) == Some("off") {
        return Ok(1.0);
    }
    let preset = settings
        .get("preset")
        .map(String::as_str)
        .filter(|s| !s.is_empty())
        .unwrap_or("0")
        .parse::<i64>()
        .map_err(|e| e.to_string())?;
    Ok(SCALES[preset.clamp(0, 4) as usize])
}
fn even(value: f64) -> u32 {
    (value.round_ties_even() as u32).min(u32::MAX / 2) * 2
}
pub fn output_size(settings: &BTreeMap<String, String>) -> (u32, u32) {
    settings
        .get("output_res")
        .and_then(|s| size(s))
        .unwrap_or(NATIVE)
}
pub fn render_size(
    settings: &BTreeMap<String, String>,
    override_size: &str,
) -> Result<Option<(u32, u32)>> {
    if !override_size.is_empty() {
        return size(override_size)
            .map(Some)
            .ok_or("Invalid render resolution".into());
    }
    let scale = scale(settings)?;
    if scale == 1.0 {
        return Ok(None);
    }
    Ok(Some((
        even(1920.0 / scale / 2.0).max(2),
        even(1080.0 / scale / 2.0).max(2),
    )))
}
pub fn hud_canvas(output: (u32, u32)) -> (u32, u32) {
    if u64::from(output.0) * 1080 <= u64::from(output.1) * 1920 {
        NATIVE
    } else {
        (
            even(1080.0 * f64::from(output.0) / f64::from(output.1) / 2.0),
            1080,
        )
    }
}
pub fn ui_viewport(output: (u32, u32), sharp: bool) -> (u32, u32) {
    let canvas = hud_canvas(output);
    if !sharp || canvas != NATIVE {
        return canvas;
    }
    let k = (f64::from(output.0) / 1920.0).min(f64::from(output.1) / 1080.0);
    if k <= 1.0 {
        canvas
    } else {
        (
            even(1920.0 * k / 2.0).min(8192),
            even(1080.0 * k / 2.0).min(8192),
        )
    }
}
pub fn scaled_sizes(
    settings: &BTreeMap<String, String>,
) -> Result<Option<((u32, u32), (u32, u32))>> {
    let out = output_size(settings);
    if out == NATIVE || settings.get("upscaler").map(String::as_str) == Some("taa") {
        return Ok(None);
    }
    let k = scale(settings)?;
    let mut render = (
        even(f64::from(out.0) / k / 2.0).max(2),
        even(f64::from(out.1) / k / 2.0).max(2),
    );
    let mut ui = vec![
        hud_canvas(out),
        ui_viewport(
            out,
            settings.get("sharp_ui").map(String::as_str) == Some("1"),
        ),
    ];
    ui.sort_unstable();
    ui.dedup();
    for size in ui {
        if render == size {
            render = (size.0 - 4, size.1 - 2);
        }
    }
    Ok(Some((render, out)))
}
const SW: u64 = 0x2196a6b - BASE;
const SH: u64 = 0x2196a7a - BASE;
const UW: u64 = 0x2358554 - BASE;
const UH: u64 = 0x235855d - BASE;
fn immediate(op: u8, value: u32) -> Result<Vec<u8>> {
    if value >= 1 << 24 {
        return Err("Resolution immediate exceeds 24 bits".into());
    }
    let mut bytes = vec![op];
    bytes.extend_from_slice(&value.to_le_bytes()[..3]);
    Ok(bytes)
}
pub fn resolution(
    patches: &[Patch],
    render: (u32, u32),
    ui: (u32, u32),
    elf: &Elf,
) -> Result<Vec<Write>> {
    let mut writes = compile(
        patches,
        &["Resolution Patch 1280x720 (16:9)".into()],
        "01.09",
        elf,
    )?;
    let replacements = BTreeMap::from([
        (SW, (0xb8, 0x500, render.0)),
        (SH, (0xb8, 0x2d0, render.1)),
        (UW, (0xb8, 0x500, ui.0)),
        (UH, (0xb9, 0x2d0, ui.1)),
    ]);
    let mut seen = BTreeSet::new();
    for w in &mut writes {
        if let Some((opcode, old, value)) = replacements.get(&w.offset) {
            if w.data != immediate(*opcode, *old)? || !seen.insert(w.offset) {
                return Err("Unexpected resolution template instruction".into());
            }
            w.data = immediate(*opcode, *value)?;
        }
    }
    if seen != replacements.keys().copied().collect() {
        return Err("Resolution template is incomplete".into());
    }
    Ok(writes)
}
pub fn ultrawide(patches: &[Patch], output: (u32, u32), elf: &Elf) -> Result<Vec<Write>> {
    let canvas = hud_canvas(output).0;
    let mut floats = BTreeMap::from([
        (
            0x183a35d - BASE,
            (2560.0 / 1080.0, f64::from(output.0) / f64::from(output.1)),
        ),
        (0x4d26ea4 - BASE, (2560.0, f64::from(canvas))),
    ]);
    for at in [0x4cf9a10, 0x4cf9a70, 0x4cf9ae0] {
        floats.insert(
            at - BASE,
            (0.5 * 1920.0 / 2560.0, 0.5 * 1920.0 / f64::from(canvas)),
        );
    }
    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    for mut w in compile(
        patches,
        &["Resolution Patch 2560x1080 (21:9)".into()],
        "01.09",
        elf,
    )? {
        if [SW, SH, UW, UH].contains(&w.offset) {
            continue;
        }
        if let Some((old, new)) = floats.get(&w.offset) {
            if w.data.len() != 4
                || (f64::from(f32::from_le_bytes(w.data.as_slice().try_into().unwrap())) - *old)
                    .abs()
                    > 1e-3
            {
                return Err("Unexpected ultrawide template float".into());
            }
            w.data = (*new as f32).to_le_bytes().to_vec();
            seen.insert(w.offset);
        } else if w.data.len() == 4
            && [0xb8, 0xb9, 0xbe].contains(&w.data[0])
            && w.data[1..] == 2560u32.to_le_bytes()[..3]
        {
            w.data = immediate(w.data[0], canvas)?;
        }
        result.push(w);
    }
    if seen != floats.keys().copied().collect() {
        return Err("Ultrawide template is incomplete".into());
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sharp_ui_and_scene_sizes_stay_distinct() {
        let mut s = BTreeMap::from([
            ("output_res".into(), "3840x2160".into()),
            ("preset".into(), "3".into()),
            ("sharp_ui".into(), "1".into()),
        ]);
        assert_eq!(
            scaled_sizes(&s).unwrap(),
            Some(((1916, 1078), (3840, 2160)))
        );
        s.insert("preset".into(), "0".into());
        assert_eq!(
            scaled_sizes(&s).unwrap(),
            Some(((3836, 2158), (3840, 2160)))
        );
        assert_eq!(hud_canvas((3440, 1440)), (2580, 1080));
        assert_eq!(parse_size(" 2401 × 1081 "), Some((2400, 1080)));
        assert_eq!(parse_size("100x100"), None);
    }
}
