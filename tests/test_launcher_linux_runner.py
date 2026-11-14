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
