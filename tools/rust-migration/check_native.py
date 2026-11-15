#!/usr/bin/env python3
"""Development-only golden comparison; never shipped/executed by the launcher."""
import argparse
import hashlib
import importlib
import json
from pathlib import Path
import struct
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts'))
import patches


def sha256(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('binary', type=Path)
    parser.add_argument('prepared', type=Path)
    parser.add_argument('--reference-prepared', type=Path)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    xml = ROOT / 'patches/Bloodborne.xml'
    elf_path = args.prepared / 'eboot.elf'
    elf = elf_path.read_bytes()
    segments = patches.eboot_segments(elf)
    import xml.etree.ElementTree as ET
    names = sorted({m.get('Name') for m in ET.parse(xml).getroot().iter('Metadata')
                    if m.get('AppVer') == '01.09' and m.get('AppElf', 'eboot.bin') == 'eboot.bin'})
    valid, rejected = [], []
    for name in names:
        try:
            patches.compile_patches(xml, [name], '01.09', segments)
            valid.append(name)
        except ValueError:
            rejected.append(name)
    cases = {'all_valid': valid, **patches.PATCH_GROUPS}
    result = {'patches': {}, 'prepared': {}, 'rejected': {}, 'native_plans': {}, 'resolution': {}}
    for index, name in enumerate(rejected):
        output = args.output / f'rejected-{index}.bin'
        output.unlink(missing_ok=True)
        process = subprocess.run([str(args.binary.resolve()), '--compile-patches', str(elf_path), str(xml),
                                  '01.09', str(output), name], text=True, capture_output=True)
        result['rejected'][name] = process.returncode != 0 and not output.exists()
    for case, selected in cases.items():
        expected = patches.compile_patches(xml, selected, '01.09', segments)
        blob = struct.pack('<8sQQ', b'BBPATCH2', patches.EBOOT_BASE, len(expected))
        for offset, data in expected:
            blob += struct.pack('<QQ', offset, len(data)) + data
        output = args.output / f'patches-{case}.bin'
        process = subprocess.run([str(args.binary.resolve()), '--compile-patches', str(elf_path), str(xml),
                                  '01.09', str(output), *selected], text=True, capture_output=True, check=True)
        report = json.loads(process.stdout)
        expected_hash = patches.patch_fingerprint(elf, xml, '01.09', selected)
        equal = output.read_bytes() == blob
        fingerprint_equal = report['fingerprint'] == expected_hash
        result['patches'][case] = {'writes': len(expected), 'binary_equal': equal,
                                   'fingerprint_equal': fingerprint_equal}
    plans = json.loads((ROOT / 'launcher-rs/resources/patch-plans.json').read_text())
    for name in plans:
        module = importlib.import_module(name)
        module.check_original(lambda start, end: patches.eboot_bytes(elf, start, end))
        expected = module.code_writes()
        blob = struct.pack('<8sQQ', b'BBPATCH2', patches.EBOOT_BASE, len(expected))
        for offset, data in expected:
            blob += struct.pack('<QQ', offset, len(data)) + data
        output = args.output / f'native-{name}.bin'
        subprocess.run([str(args.binary.resolve()), '--compile-static-patches', str(elf_path),
                        str(output), name], text=True, capture_output=True, check=True)
        result['native_plans'][name] = output.read_bytes() == blob
    for render, output, sharp in [((1280, 720), (1920, 1080), False),
                                  ((1916, 1078), (3840, 2160), True),
                                  ((1720, 720), (3440, 1440), False),
                                  ((2560, 720), (5120, 1440), True)]:
        viewport = patches.ui_viewport(output, {'sharp_ui': '1' if sharp else '0'})
        expected = patches.resolution_writes(xml, render, '01.09', segments, viewport)
        if patches.hud_canvas(output) != patches.OUTPUT_SIZE:
            expected += patches.ultrawide_writes(xml, output, '01.09', segments)
        blob = struct.pack('<8sQQ', b'BBPATCH2', patches.EBOOT_BASE, len(expected))
        for offset, data in expected:
            blob += struct.pack('<QQ', offset, len(data)) + data
        label = f'{render[0]}x{render[1]}-{output[0]}x{output[1]}-sharp{int(sharp)}'
        target = args.output / (label + '.bin')
        subprocess.run([str(args.binary.resolve()), '--compile-resolution-patches', str(elf_path), str(xml),
                        str(target), f'{render[0]}x{render[1]}', f'{output[0]}x{output[1]}', str(int(sharp))],
                       text=True, capture_output=True, check=True)
        result['resolution'][label] = target.read_bytes() == blob
    if args.reference_prepared:
        for name in ('boot.bin', 'boot-linked.bin', 'eboot.elf', 'entry.bin'):
            a, b = args.prepared / name, args.reference_prepared / name
            result['prepared'][name] = a.stat().st_size == b.stat().st_size and sha256(a) == sha256(b)
        a = json.loads((args.prepared / 'link.json').read_text())
        b = json.loads((args.reference_prepared / 'link.json').read_text())
        for name in ('bindings', 'imports', 'fs_loads_patched', 'cpu_compat', 'red_zone'):
            result['prepared']['report:' + name] = a[name] == b[name]
    success = (all(r['binary_equal'] and r['fingerprint_equal'] for r in result['patches'].values())
               and all(result['prepared'].values()) and all(result['rejected'].values())
               and all(result['native_plans'].values()) and all(result['resolution'].values()))
    result['passed'] = success
    (args.output / 'native-comparison.json').write_text(json.dumps(result, indent=2))
    print(json.dumps(result))
    raise SystemExit(0 if success else 1)


if __name__ == '__main__':
    main()
