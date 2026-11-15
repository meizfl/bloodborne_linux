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
    def closeable_launcher(self, process=None):
        window = object.__new__(launcher.Launcher)
        window.root = mock.Mock()
        window.app = {}
        window.process = process
        window.messagebox = mock.Mock()
        window.test_running = lambda: False
        window.window_place = lambda: ''
        window.collect = mock.Mock()
        window.drop_tray = mock.Mock()
        return window

    def test_linux_close_destroys_idle_window_without_steam_dialog(self):
        window = self.closeable_launcher()
        with mock.patch.object(launcher, 'WINDOWS', False):
            window.close()
        window.root.destroy.assert_called_once()
        window.messagebox.showinfo.assert_not_called()

    def test_linux_close_keeps_running_game_and_hides_its_launcher(self):
        window = self.closeable_launcher(process=mock.Mock())
        with mock.patch.object(launcher, 'WINDOWS', False):
            window.close()
        window.root.withdraw.assert_called_once()
        window.root.destroy.assert_not_called()
        window.messagebox.showinfo.assert_not_called()
        self.assertTrue(window.hidden)

    def test_linux_steam_notice_belongs_only_to_the_steam_action(self):
        window = self.closeable_launcher()
        with mock.patch.object(launcher, 'WINDOWS', False):
            window.add_to_steam()
        window.messagebox.showinfo.assert_called_once()
        window.root.destroy.assert_not_called()

    def test_portable_mode_is_opt_in_and_uses_the_executable_directory(self):
        with tempfile.TemporaryDirectory(prefix='portable launcher ') as folder, \
                mock.patch.object(host, 'family', return_value='linux'):
            root = Path(folder).resolve()
            env = {'XDG_CONFIG_HOME': '/global'}
            self.assertFalse(host.configure_portable(root / 'BloodborneLauncher', True, env))
            (root / 'portable-mode').touch()
            self.assertTrue(host.configure_portable(root / 'BloodborneLauncher', True, env))
            self.assertNotIn('BB_DATA_DIR', env)
            self.assertEqual(env['XDG_CONFIG_HOME'], '/global')
            self.assertEqual(host.config_dir(env=env), Path.home() / '.local/share/Bloodborne')

    def test_frozen_linux_normal_start_does_not_call_windows_updater(self):
        calls = mock.Mock()
        with mock.patch.object(launcher, 'WINDOWS', False), mock.patch.object(launcher, 'FROZEN', True), \
                mock.patch.object(sys, 'argv', ['BloodborneLauncher']), \
                mock.patch.object(launcher, 'load_settings', return_value={}), \
                mock.patch.object(launcher, 'restore_benchmark_backup'), \
                mock.patch.object(launcher, 'one_launcher', return_value=True), \
                mock.patch.object(launcher, 'bring_to_front_on_request'), \
                mock.patch.object(launcher, 'Launcher', calls.Launcher), \
                mock.patch.object(launcher.updater, 'underway', side_effect=AssertionError('WinAPI called')), \
                mock.patch.object(launcher.updater, 'clear_leftovers', side_effect=AssertionError('WinAPI called')), \
                mock.patch('tkinter.Tk', return_value=calls.root), \
                mock.patch.object(launcher, 'LANG', launcher.LANG), mock.patch.dict(launcher.saves.WORDS):
            launcher.main()
        calls.Launcher.assert_called_once()
        calls.root.mainloop.assert_called_once()

    def test_windows_uses_appdata_for_settings(self):
        self.assertEqual(host.config_dir({'APPDATA': '/roaming'}, '/home/test', 'windows'),
                         Path('/roaming/Bloodborne'))

    def test_linux_uses_xdg_and_keeps_gtk_profile_separate(self):
        self.assertEqual(host.config_dir({'XDG_CONFIG_HOME': '/config'}, '/home/test', 'linux'),
                         Path('/home/test/.local/share/Bloodborne'))
        self.assertEqual(host.data_dir({'XDG_DATA_HOME': '/data'}, '/home/test', 'linux'), Path('/home/test/.local/share/Bloodborne'))
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
            (port / 'unused').touch()
            code = ('import sys; sys.path.insert(0, sys.argv[1]); import bbport_platform as h; '
                    'import bbport_linux_runner as r; import time; '
                    'r.main=lambda *a: (print("ready", flush=True), time.sleep(60))[1]; '
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


class DesktopBrowserTests(unittest.TestCase):
    def test_linux_url_uses_desktop_opener(self):
        import bbport_browser
        with mock.patch.object(bbport_browser.sys, 'platform', 'linux'), \
                mock.patch.object(host.subprocess, 'Popen') as spawn, \
                mock.patch.dict(host.os.environ, {'XDG_CONFIG_HOME': '/system/profile',
                                                 'LD_LIBRARY_PATH': '/bundle',
                                                 'LD_LIBRARY_PATH_ORIG': '/system/libs'}), \
                mock.patch.object(host.sys, 'frozen', True, create=True):
            self.assertTrue(bbport_browser.open('https://bbport.de/'))
        self.assertEqual(spawn.call_args.args[0], ['xdg-open', 'https://bbport.de/'])
        env = spawn.call_args.kwargs['env']
        self.assertEqual(env['XDG_CONFIG_HOME'], '/system/profile')
        self.assertEqual(env['LD_LIBRARY_PATH'], '/system/libs')


class UserDataMigrationTests(unittest.TestCase):
    def test_migration_copies_saves_and_preserves_existing_settings_and_source(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            source, target = root / 'install', root / 'profile'
            (source / 'user').mkdir(parents=True)
            (source / 'user/save.bin').write_bytes(b'character')
            (source / 'bbport.ini').write_text('old settings')
            target.mkdir()
            (target / 'bbport.ini').write_text('current settings')
            with mock.patch.object(host, 'data_dir', return_value=target), \
                    mock.patch.object(host.Path, 'home', return_value=root), \
                    mock.patch.dict(host.os.environ, {}, clear=True):
                host.migrate_user_data(source)
                self.assertEqual((target / 'user/save.bin').read_bytes(), b'character')
                self.assertEqual((target / 'bbport.ini').read_text(), 'current settings')
                self.assertEqual((source / 'user/save.bin').read_bytes(), b'character')
                (target / 'user/save.bin').write_bytes(b'new character')
                host.migrate_user_data(source)
                self.assertEqual((target / 'user/save.bin').read_bytes(), b'new character')


@unittest.skipUnless(sys.platform.startswith('linux'), 'Linux ELF loader environment')
class NativeLibraryEnvironmentTests(unittest.TestCase):
    def test_desktop_apps_drop_only_the_bundled_font_configuration(self):
        bundled = {'FONTCONFIG_FILE': '/bundle/fonts.conf', 'BB_BUNDLED_FONTCONFIG': '/bundle/fonts.conf'}
        self.assertNotIn('FONTCONFIG_FILE', host.desktop_environment(bundled))
        bundled['FONTCONFIG_FILE'] = '/user/fonts.conf'
        result = host.desktop_environment(bundled)
        self.assertEqual(result['FONTCONFIG_FILE'], '/user/fonts.conf')
        self.assertNotIn('BB_BUNDLED_FONTCONFIG', result)

    def test_external_apps_do_not_load_the_launchers_wayland_plugins(self):
        env = {'LIBDECOR_PLUGIN_DIR': '/bundle/plugins', 'BB_BUNDLED_LIBDECOR': '/bundle/plugins',
               'XLOCALEDIR': '/bundle/locale', 'BB_BUNDLED_XLOCALE': '/bundle/locale'}
        self.assertEqual(host.desktop_environment(env), {})
        env['LIBDECOR_PLUGIN_DIR'] = '/user/plugins'
        self.assertEqual(host.desktop_environment(env), {'LIBDECOR_PLUGIN_DIR': '/user/plugins'})

    def test_frozen_native_process_does_not_inherit_python_cpp_libraries(self):
        env = {'LD_LIBRARY_PATH': '/python-bundle', 'LD_LIBRARY_PATH_ORIG': '/host-libraries', 'BB_ONLINE': '0'}
        with mock.patch.object(host.sys, 'frozen', True, create=True):
            result = host.native_environment('/install/game_runtime/bb-probe', env)
        self.assertEqual(result['LD_LIBRARY_PATH'], '/install/game_runtime/libs:/host-libraries')
        self.assertEqual(result['BB_ONLINE'], '0')
        self.assertEqual(env['LD_LIBRARY_PATH'], '/python-bundle')


class SmoothWaylandWheelTests(unittest.TestCase):
    def window(self):
        window = object.__new__(launcher.Launcher)
        window.root = mock.Mock()
        window.current_page = window.wheel_page = 'controls'
        window.wheel_last, window.wheel_pixels = 0.0, 64.0
        window.pages = {'controls': mock.Mock(), 'play': mock.Mock()}
        return window

    def test_wheel_motion_is_spread_over_frames_without_losing_distance(self):
        window = self.window()
        for frame in range(1, 100):
            with mock.patch.object(launcher.time, 'monotonic', return_value=frame * .008):
                window.flush_smooth_wheel()
            if frame == 2:
                window.wheel_pixels += 64  # another event during the animation
        steps = [call.args[0] for call in window.pages['controls'].scroll.call_args_list]
        self.assertGreater(len(steps), 3)
        self.assertTrue(all(0 < step < 64 for step in steps))
        self.assertAlmostEqual(sum(steps) + window.wheel_pixels + window.wheel_fraction, 128)
        self.assertLess(window.wheel_pixels, 1)

    def test_new_notch_preserves_velocity_and_starts_without_a_jump(self):
        window = self.window()
        with mock.patch.object(launcher.time, 'monotonic', return_value=.008):
            window.flush_smooth_wheel()
        self.assertLess(window.wheel_fraction, 1)
        window.pages['controls'].scroll.assert_not_called()
        velocity = window.wheel_velocity
        window.wheel_pixels += 64
        self.assertEqual(window.wheel_velocity, velocity)
        with mock.patch.object(launcher.time, 'monotonic', return_value=.016):
            window.flush_smooth_wheel()
        self.assertGreater(window.wheel_velocity, velocity)

    def test_switching_pages_cancels_pending_motion(self):
        window = self.window()
        window.current_page = 'play'
        window.flush_smooth_wheel()
        window.pages['play'].scroll.assert_not_called()
        self.assertEqual(window.wheel_pixels, 0)
        self.assertIsNone(window.wheel_pending)

    def test_reaching_an_edge_discards_remaining_inertia(self):
        window = self.window()
        window.pages['controls'].scroll.return_value = False
        with mock.patch.object(launcher.time, 'monotonic', return_value=.04):
            window.flush_smooth_wheel()
        self.assertEqual(window.wheel_pixels, 0)
        self.assertEqual(window.wheel_velocity, 0)
        self.assertIsNone(window.wheel_pending)
