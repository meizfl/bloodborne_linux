#!/usr/bin/env python3
"""Export source-owned input names/defaults for the Rust parser (no game/profile input)."""
import json
from pathlib import Path
import sys
ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'launcher'))
import bbport_input_config as inputs
fields = ('KEYBOARD_OUTPUTS', 'BUTTON_OUTPUTS', 'STICK_OUTPUTS', 'KEY_NAMES', 'MOUSE_BUTTON_NAMES',
          'MODIFIER_NAMES', 'MAX_COMBO', 'SLOTS', 'DISPLAY_NAMES', 'CONTROLLER_OUTPUTS',
          'CONTROLLER_NAMES', 'CONTROLLER_ALIASES', 'DEFAULT_CONTROLLER', 'MOUSE_SETTINGS_DEFAULTS',
          'DEFAULT_INPUT_INI', 'DS3_LAYOUT', 'TK_KEYSYM_TO_NAME', 'TK_SHIFTED_KEYSYM_TO_NAME',
          'TK_BUTTON_NUM_TO_NAME', 'ALWAYS_RESERVED', 'CONTROLLER_DISPLAY')
(ROOT / 'launcher-rs/resources/input-spec.json').write_text(
    json.dumps({name: getattr(inputs, name) for name in fields}, ensure_ascii=False, indent=2, default=lambda value: sorted(value) if isinstance(value, set) else str(value)) + '\n')
