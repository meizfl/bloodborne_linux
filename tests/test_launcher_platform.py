# SPDX-License-Identifier: GPL-2.0-or-later
"""Windows compatibility and real POSIX process-lifecycle checks for the shared UI."""
from pathlib import Path
import os
import signal
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / 'launcher'))
import bbport_platform as host
import bbport_launcher_win as launcher


class PlatformTests(unittest.TestCase):
    def test_windows_keeps_existing_settings_location(self):
        self.assertEqual(host.config_dir({'APPDATA': '/roaming'}, '/home/test', 'windows'),
                         Path('/roaming/bbport-launcher'))

    def test_linux_uses_xdg_and_keeps_gtk_profile_separate(self):
        self.assertEqual(host.config_dir({'XDG_CONFIG_HOME': '/config'}, '/home/test', 'linux'),
                         Path('/config/bbport-launcher-tk'))
        self.assertEqual(host.data_dir({'XDG_DATA_HOME': '/data'}, '/home/test', 'linux'), Path('/data/bbport'))
        self.assertEqual(host.data_dir({'BB_DATA_DIR': '/custom'}, '/home/test', 'linux'), Path('/custom'))

    def test_no_windows_dll_is_loaded_when_importing_on_linux(self):
        if os.name != 'nt':
            self.assertIsNone(launcher.bbport_tray)
            self.assertIsNone(launcher.bbport_frame)

    def test_windows_source_command_is_unchanged(self):
        with mock.patch.object(launcher, 'WINDOWS', True), mock.patch.object(launcher, 'FROZEN', False):
            self.assertEqual(launcher.run_command(), [sys.executable, str(launcher.PORT_DIR / 'run.py')])

    def test_linux_uses_shared_runner_role_without_external_setsid(self):
        with mock.patch.object(launcher, 'WINDOWS', False), mock.patch.object(launcher, 'FROZEN', False):
            command = launcher.run_command()
            self.assertEqual(command[0], sys.executable)
            self.assertEqual(command[-1], '--run')
            self.assertNotIn('setsid', command)

    def test_windows_probe_results_still_pass_through(self):
        with mock.patch.object(launcher, 'WINDOWS', True), \
                mock.patch.object(launcher.bbport_fsr41, 'query', return_value={'native': True}) as query:
            self.assertEqual(launcher.query_fsr_support(ROOT, {}, refresh=True), {'native': True})
            query.assert_called_once_with(ROOT, {}, refresh=True)

    def test_linux_reports_windows_providers_unavailable(self):
        with mock.patch.object(launcher, 'WINDOWS', False):
            result = launcher.query_fsr_support(ROOT)
            self.assertFalse(result['dx12']['supported'])
            self.assertEqual(result['dx12']['reason'], 'driver')
            self.assertIn('Windows', result['dx12']['detail'])

    def test_linux_fullscreen_can_be_turned_off_after_fullscreen_launch(self):
        settings = {**launcher.APP_DEFAULTS, 'game_dir': '/game', 'online': False, 'fullscreen': False}
        with mock.patch.object(launcher, 'WINDOWS', False), \
                mock.patch.dict(os.environ, {'BB_WINDOW_MODE': 'borderless'}):
            env = launcher.game_environment(settings, dict(launcher.INI_DEFAULTS))
        self.assertEqual(env['BB_WINDOW_MODE'], 'windowed')
        self.assertEqual(env['BB_FULLSCREEN'], '0')
        self.assertEqual(env['BB_PREBUILT'], '1')

    def test_missing_loader_has_actionable_error(self):
        with tempfile.TemporaryDirectory() as folder, mock.patch.object(host, 'family', return_value='linux'), \
                mock.patch.object(host.platform, 'machine', return_value='x86_64'):
            self.assertIn('build.sh', host.game_problem(folder, {}))


@unittest.skipIf(os.name == 'nt', 'POSIX sessions and flock')
class PosixProcessTests(unittest.TestCase):
    def test_stop_terminates_only_the_launch_process_group(self):
        process = subprocess.Popen([sys.executable, '-c', 'import time; print("ready", flush=True); time.sleep(60)'],
                                   stdout=subprocess.PIPE, text=True, start_new_session=True)
        try:
            self.assertEqual(process.stdout.readline().strip(), 'ready')
            host.GameJob(process).terminate()
            self.assertEqual(process.wait(timeout=5), -signal.SIGTERM)
        finally:
            if process.poll() is None:
                process.kill()
                process.wait()
            process.stdout.close()

    def test_preparation_lock_survives_exec_and_blocks_second_launch(self):
        with tempfile.TemporaryDirectory() as folder:
            port, config = Path(folder), Path(folder) / 'config'
            (port / 'run.sh').write_text('echo ready\nsleep 60\n', encoding='utf-8')
            code = ('import sys; sys.path.insert(0, sys.argv[1]); import bbport_platform as h; '
                    'raise SystemExit(h.run_game(sys.argv[2], sys.argv[3], []))')
            command = [sys.executable, '-c', code, str(ROOT / 'launcher'), str(port), str(config)]
            first = subprocess.Popen(command, stdout=subprocess.PIPE, text=True, start_new_session=True)
            try:
                self.assertEqual(first.stdout.readline().strip(), 'ready')
                self.assertTrue(host.game_locked(config))
                second = subprocess.run(command, capture_output=True, text=True, timeout=5)
                self.assertEqual(second.returncode, 1)
                self.assertIn('already', second.stderr)
            finally:
                host.GameJob(first).terminate()
                first.wait(timeout=5)
                first.stdout.close()
            self.assertFalse(host.game_locked(config))


if __name__ == '__main__':
    unittest.main()
