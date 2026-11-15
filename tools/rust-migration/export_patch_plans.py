#!/usr/bin/env python3
"""One-time export of source-owned literal patch plans; reads no game files."""
import argparse
import importlib
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts'))


class ComparedBytes:
    def __init__(self, guards, start, end):
        self.guards, self.start, self.end = guards, start, end

    def __eq__(self, expected):
        assert isinstance(expected, bytes) and len(expected) == self.end - self.start
        self.guards.append({'offset': self.start, 'bytes': expected.hex()})
        return True

    def __ne__(self, expected):
        return not self == expected


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, default=ROOT / 'launcher-rs/resources/patch-plans.json')
    args = parser.parse_args()
    result = {}
    for name in ('quit_menu', 'pc_menu', 'rebirth', 'cloth_time', 'cloth', 'foliage_time', 'air_drag', 'effect_time'):
        module = importlib.import_module(name)
        guards = []
        module.check_original(lambda start, end: ComparedBytes(guards, start, end))
        assert guards, name
        result[name] = {'guards': guards, 'writes': [{'offset': at, 'bytes': data.hex()}
                                                   for at, data in module.code_writes()]}
        if name in ('quit_menu', 'rebirth'):
            import quit_menu
            result[name]['code_end'] = quit_menu.CODE_END
            result[name]['cave_end'] = quit_menu.CAVE_END
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n')
    print(json.dumps({name: len(plan['writes']) for name, plan in result.items()}))


if __name__ == '__main__':
    main()
