# SPDX-License-Identifier: GPL-2.0-or-later
"""Linux must resolve bundled artwork fonts instead of system substitutions."""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'launcher'))


@unittest.skipUnless(sys.platform.startswith('linux'), 'Fontconfig registration is Linux specific')
class BundledFontsTests(unittest.TestCase):
    def test_tk_resolves_bundled_families(self):
        import tkinter as tk
        import tkinter.font as tkfont
        import bbport_theme as theme
        root = tk.Tk()
        try:
            theme.load_fonts(root)
            for family in (theme.HEAD, theme.BODY, theme.ICONS):
                with self.subTest(family=family):
                    resolved = tkfont.Font(root=root, family=family).actual('family')
                    self.assertEqual(resolved, family)
            self.assertEqual(theme.HEAD, 'Cinzel SemiBold')
            self.assertEqual(theme.BODY, 'Inter')
            self.assertIn('FluentSystemIcons', theme.ICONS)
            self.assertEqual(theme.ICON['online'], theme.PORTABLE_ICON['online'])
            self.assertNotEqual(theme.ICON['online'], theme.ICON['seamless'])
        finally:
            root.destroy()
