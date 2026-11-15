# SPDX-License-Identifier: GPL-2.0-or-later
"""The embedded runner owns restart/cleanup without external shell wrappers."""
import sys
from pathlib import Path
import unittest
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'launcher'))
import bbport_linux_runner as runner


class EmbeddedRunnerTests(unittest.TestCase):
    def test_native_restart_reprepares_and_cleans_each_overlay(self):
        first, second = mock.Mock(), mock.Mock()
        first.wait.return_value = 85
        second.wait.return_value = 0
        command = ['/install/game_runtime/bb-probe', 'boot.bin']
        with mock.patch.object(runner, 'prepare', side_effect=[
                (command, Path('/cache/mod-game-first'), Path('/game')),
                (command, Path('/cache/mod-game-second'), Path('/game'))]) as prepare, \
                mock.patch.object(runner.subprocess, 'Popen', side_effect=[first, second]) as spawn, \
                mock.patch.object(runner, 'cleanup') as cleanup:
            self.assertEqual(runner.main('/install'), 0)
        self.assertEqual(prepare.call_count, 2)
        self.assertEqual(cleanup.call_count, 2)
        self.assertEqual(spawn.call_args.args[0][0], '/install/game_runtime/bb-probe')
        self.assertEqual(spawn.call_args.kwargs['env']['BB_LAUNCHER_MANAGED'], '1')
        self.assertNotIn('shell', spawn.call_args.kwargs)

    def test_overlay_is_cleaned_when_native_launch_fails(self):
        with mock.patch.object(runner, 'prepare', return_value=(['missing'], Path('/overlay'), Path('/game'))), \
                mock.patch.object(runner.subprocess, 'Popen', side_effect=OSError('missing')), \
                mock.patch.object(runner, 'cleanup') as cleanup:
            with self.assertRaises(OSError):
                runner.main('/install')
        cleanup.assert_called_once_with(Path('/overlay'), Path('/game'))


@unittest.skipUnless(sys.platform.startswith('linux'), 'Linux preparation')
class ResolutionDecisionTests(unittest.TestCase):
    def test_ui_decision_can_force_startup_resolution_over_live_request(self):
        import tempfile
        import json
        sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
        import linux_start
        import patches
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            game, data = root / 'Game', root / 'profile'
            game.mkdir()
            (game / 'eboot.bin').touch()
            (data / 'out').mkdir(parents=True)
            (data / 'out/link.json').write_text(json.dumps({}))

            def step(name, _function, *args):
                results = args[-1]
                if name == 'before-view':
                    results.add('groups', 'fps,resolution,effects')
                elif name == 'ui-res':
                    results.add('live', '0')  # Sharp UI/ultrawide needs the startup patch.

            with mock.patch.dict(runner.os.environ, {'BB_GAME_DIR': str(game), 'BB_DATA_DIR': str(data),
                                                      'BB_LIVE_RES': '1'}, clear=True), \
                    mock.patch.object(linux_start, 'attempt', side_effect=step), \
                    mock.patch.object(patches, 'read_settings', return_value={}), \
                    mock.patch.object(patches, 'scaled_sizes', return_value=((1720, 720), (3440, 1440))), \
                    mock.patch.object(runner, 'script', side_effect=lambda name, *a, **k: str(game)
                                      if name == 'mods.py' else None):
                runner.prepare(root)
                self.assertEqual(runner.os.environ['BB_RENDER_RES'], '1720x720')
                self.assertEqual(runner.os.environ['BB_OUTPUT_RES'], '3440x1440')
                self.assertTrue(Path(runner.os.environ['BB_GPU_USER_DIR']).is_dir())
                self.assertEqual(Path(runner.os.environ['BB_GPU_USER_DIR']), data / 'user')
