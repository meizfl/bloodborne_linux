#!/usr/bin/env python3
"""Compare Rust input.ini reading/rewriting with the source implementation."""
import argparse
import json
from pathlib import Path
import subprocess
import sys
ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'launcher'))
import bbport_input_config as inputs


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('binary', type=Path)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    cases = {
        'default': inputs.DEFAULT_INPUT_INI,
        'mixed': '# keep\ncross = cross\ncross = e # first\ncross = q\nr3 = c\nr3 = q\n'
                 'future = a,b\n# mouse_to_joystick = left\nmouse_movement_params = .2, 2.5, .3\n',
        'whitespace-and-tags': 'c r o s s :any = lctrl , leftbutton :any # tail\n'
                               'axis_left_x_plus = leftbutton\naxis_left_x_plus = d\n'
                               'cross = circle\ncross = triangle\nmouse_invert_x = on\n'
                               'mouse_sensitivity_y = 2.5\nmouse_camera = stick\n',
        'bom': '\ufeffcross = e\noptions = options\nhotkey_reload_inputs = f4\n',
    }
    edits = {'keyboard': {'cross': ['lctrl, leftbutton', 'f3'], 'r3': None,
                           'axis_left_x_plus': []}, 'controller': {'cross': 'square', 'options': None}}
    request = args.output / 'edits.json'
    request.write_text(json.dumps(edits))
    result = {}
    for name, text in cases.items():
        source = args.output / (name + '.ini')
        source.write_text(text)
        bindings, lines, toggle, reload, mouse = inputs.load_input_ini(source)
        expected = {'bindings': bindings, 'controller': inputs.load_controller_bindings(lines),
                    'toggle_key': toggle, 'reload_key': reload, 'mouse': mouse}
        process = subprocess.run([str(args.binary.resolve()), '--inspect-input', str(source)],
                                 text=True, capture_output=True, check=True)
        actual = json.loads(process.stdout)
        reference = args.output / (name + '-reference.ini')
        inputs.save_input_ini(reference, edits['keyboard'], lines)
        inputs.save_controller_bindings(reference, edits['controller'], reference.read_text().splitlines())
        output = args.output / (name + '-rust.ini')
        subprocess.run([str(args.binary.resolve()), '--rewrite-input', str(source), str(output), str(request)],
                       text=True, capture_output=True, check=True)
        result[name] = {'read_equal': actual == expected,
                        'write_equal': reference.read_bytes() == output.read_bytes()}
    # The layout's comments, controller lines and mouse numbers must round-trip too.
    for name in cases:
        source = args.output / (name + '.ini')
        _, lines, _, _, _ = inputs.load_input_ini(source)
        reference = args.output / (name + '-layout-reference.ini')
        lines = inputs.apply_layout(lines)
        inputs.save_input_ini(reference, edits['keyboard'], lines)
        inputs.save_controller_bindings(reference, edits['controller'], reference.read_text().splitlines())
        _, lines, _, _, mouse = inputs.load_input_ini(reference)
        mouse_edits = {'enabled': True, 'stick': 'left', 'speed': 3.14159, 'deadzone_offset': 0.23456,
                       'invert_x': True, 'sensitivity_y': 1.23456, 'camera': 'direct'}
        mouse.update(mouse_edits)
        inputs.save_mouse_settings(reference, mouse, lines)
        request.write_text(json.dumps({**edits, 'layout': 'ds3', 'mouse': mouse_edits}))
        output = args.output / (name + '-layout-rust.ini')
        subprocess.run([str(args.binary.resolve()), '--rewrite-input', str(source), str(output), str(request)],
                       text=True, capture_output=True, check=True)
        result[name + '-layout-mouse'] = {'read_equal': True,
                                         'write_equal': reference.read_bytes() == output.read_bytes()}
    result['passed'] = all(v['read_equal'] and v['write_equal'] for v in result.values())
    (args.output / 'input-comparison.json').write_text(json.dumps(result, indent=2))
    print(json.dumps(result))
    raise SystemExit(0 if result['passed'] else 1)


if __name__ == '__main__':
    main()
