# SPDX-License-Identifier: GPL-2.0-or-later
"""Host services for the shared Tk UI. Native Windows integrations remain optional.

Importing this module never loads a platform library or touches the filesystem.
"""
import locale
import os
from pathlib import Path
import platform
import signal
import subprocess
import sys
import time


def family(name=None):
    name = sys.platform if name is None else name
    if name == 'win32':
        return 'windows'
    return 'linux' if name.startswith('linux') else 'posix'


def configure_portable(executable=None, frozen=None, env=None):
    """Opt-in local Linux profiles, applied before the UI module reads its paths."""
    frozen = getattr(sys, 'frozen', False) if frozen is None else frozen
    if not frozen or family() != 'linux':
        return False
    root = Path(executable or sys.executable).resolve().parent
    if not (root / 'portable-mode').is_file():
        return False
    env = os.environ if env is None else env
    env.update(BB_PORT_DIR=str(root))
    return True


def config_dir(env=None, home=None, host=None):
    env, home, host = os.environ if env is None else env, Path.home() if home is None else Path(home), host or family()
    if env.get('BB_LAUNCHER_CONFIG_HOME'):
        return Path(env['BB_LAUNCHER_CONFIG_HOME'])
    return data_dir(env, home, host)


def data_dir(env=None, home=None, host=None):
    env, home, host = os.environ if env is None else env, Path.home() if home is None else Path(home), host or family()
    if env.get('BB_DATA_DIR'):
        return Path(env['BB_DATA_DIR']).expanduser()
    if host == 'windows':
        return Path(env.get('LOCALAPPDATA') or env.get('APPDATA') or home / 'AppData/Local') / 'Bloodborne'
    return home / '.local/share/Bloodborne'


def migrate_user_data(installation):
    """Copy legacy local data once, without overwriting a user's existing profile."""
    import shutil
    target = data_dir()
    installation = Path(installation)
    if os.environ.get('BB_DATA_DIR') or (target / '.migration-complete').exists():
        return
    target.mkdir(parents=True, exist_ok=True)
    for name in ('user', 'bbport.ini', 'input.ini', 'mods', 'mods.json', 'patches.json'):
        source, destination = installation / name, target / name
        if source.exists() and not destination.exists():
            if source.is_dir():
                temporary = target / ('.migrate-' + name)
                shutil.rmtree(temporary, ignore_errors=True)
                shutil.copytree(source, temporary)
                temporary.rename(destination)
            else:
                shutil.copy2(source, destination)
    for source in (installation / 'config/bbport-launcher-tk/settings.json',
                   Path.home() / '.config/bbport-launcher-tk/settings.json',
                   Path(os.environ.get('APPDATA', str(Path.home()))) / 'bbport-launcher/settings.json'):
        if source.is_file() and not (target / 'settings.json').exists():
            shutil.copy2(source, target / 'settings.json')
    (target / '.migration-complete').write_text(str(installation), encoding='utf-8')


def language():
    value = os.environ.get('LANGUAGE', '').split(':')[0] or locale.getlocale()[0] or 'en'
    return value.replace('-', '_').split('_')[0].lower()


def open_path(path, reveal=False):
    path = str(Path(path).expanduser().absolute())
    if family() == 'windows':
        if reveal:
            return subprocess.Popen(['explorer', '/select,', path])
        return os.startfile(path)
    return subprocess.Popen(['xdg-open', str(Path(path).parent) if reveal else path],
                            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, env=desktop_environment())


def desktop_environment():
    """External desktop apps must not inherit PyInstaller's private libraries."""
    env = os.environ.copy()
    if getattr(sys, 'frozen', False):
        original = env.get('LD_LIBRARY_PATH_ORIG')
        if original is None:
            env.pop('LD_LIBRARY_PATH', None)
        else:
            env['LD_LIBRARY_PATH'] = original
    return env


def open_url(url):
    subprocess.Popen(['xdg-open', url], env=desktop_environment(),
                     stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    return True


def cpu_name():
    try:
        return next(line.split(':', 1)[1].strip() for line in Path('/proc/cpuinfo').read_text().splitlines()
                    if line.startswith('model name'))
    except (OSError, StopIteration):
        return platform.processor()


def memory_gb():
    try:
        line = next(line for line in Path('/proc/meminfo').read_text().splitlines()
                    if line.startswith('MemTotal:'))
        return round(int(line.split()[1]) / 1024 ** 2)
    except (OSError, StopIteration, ValueError):
        return None


def game_pids():
    if family() == 'linux':
        found = []
        for entry in Path('/proc').iterdir():
            if not entry.name.isdigit():
                continue
            try:
                if entry.stat().st_uid == os.getuid() and (entry / 'comm').read_text().strip() == 'bb-probe':
                    found.append(int(entry.name))
            except OSError:
                pass
        return found
    try:
        output = subprocess.check_output(['ps', '-u', str(os.getuid()), '-o', 'pid=,comm='], text=True, timeout=3)
        return [int(pid) for line in output.splitlines() for pid, command in [line.strip().split(None, 1)]
                if Path(command).name == 'bb-probe']
    except (OSError, ValueError, subprocess.SubprocessError):
        return []


_launcher_lock = None


def one_launcher(config):
    import fcntl
    global _launcher_lock
    Path(config).mkdir(parents=True, exist_ok=True)
    stream = open(Path(config) / 'launcher.lock', 'a')
    try:
        fcntl.flock(stream, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError:
        stream.close()
        (Path(config) / 'launcher.front').touch()
        return False
    _launcher_lock = stream
    (Path(config) / 'launcher.front').unlink(missing_ok=True)
    return True


def front_requests(config, on_request):
    import threading
    marker = Path(config) / 'launcher.front'
    def poll():
        while True:
            time.sleep(0.25)
            try:
                marker.unlink()
            except FileNotFoundError:
                continue
            except OSError:
                return
            on_request()
    thread = threading.Thread(target=poll, daemon=True)
    thread.start()
    return thread


def wait_for_exit(pid, seconds=15):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        try:
            os.kill(int(pid), 0)
        except ProcessLookupError:
            return
        time.sleep(0.1)


def runtime_file(port, name):
    root = Path(port)
    return next((root / folder / name for folder in ('game_runtime', 'bin', 'out')
                 if (root / folder / name).is_file()), root / 'out' / name)


def game_problem(port, env=None):
    env = os.environ if env is None else env
    if family() != 'linux' or platform.machine().lower() not in ('x86_64', 'amd64'):
        return 'The native Linux game runtime requires an x86-64 Linux system.'
    probe = env.get('BB_PROBE')
    candidates = [Path(probe)] if probe else [Path(port) / folder / 'bb-probe' for folder in ('game_runtime', 'bin', 'out')]
    if not any(path.is_file() and os.access(path, os.X_OK) for path in candidates):
        return 'Native game loader not found. Build the game with bash build.sh or set BB_PROBE.'
    return None


def game_command(port, env=None):
    return ['bash', str(Path(port) / 'run.sh')]


def desktop_shortcut(command, port, data):
    """Create a Linux application entry; no shell is involved in its Exec line."""
    import shutil
    try:
        directory = subprocess.check_output(['xdg-user-dir', 'DESKTOP'], text=True, timeout=3).strip()
    except (OSError, subprocess.SubprocessError):
        directory = ''
    desktop = Path(directory) if directory else Path.home() / 'Desktop'
    desktop.mkdir(parents=True, exist_ok=True)
    def quote(value):
        return '"' + str(value).replace('%', '%%').replace('\\', '\\\\').replace('"', '\\"').replace('`', '\\`').replace('$', '\\$') + '"'
    icon = Path(data) / 'launcher.png'
    artwork = Path(getattr(sys, '_MEIPASS', port)) / 'launcher/bloodborne.png'
    if artwork.is_file():
        icon.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(artwork, icon)
    args = ['env', 'BB_DATA_DIR=' + str(data)]
    if os.environ.get('XDG_CONFIG_HOME'):
        args.append('XDG_CONFIG_HOME=' + os.environ['XDG_CONFIG_HOME'])
    args += list(command)
    text = ('[Desktop Entry]\nType=Application\nName=Bloodborne\nTerminal=false\n'
            'Exec=' + ' '.join(quote(arg) for arg in args) + '\n'
            'Path=' + str(Path(port).resolve()) + '\n'
            'Icon=' + str(icon) + '\n')
    target = desktop / 'Bloodborne.desktop'
    target.write_text(text, encoding='utf-8')
    target.chmod(0o755)
    applications = Path(os.environ.get('XDG_DATA_HOME') or Path.home() / '.local/share') / 'applications'
    applications.mkdir(parents=True, exist_ok=True)
    shutil.copy2(target, applications / target.name)
    return target


def game_locked(config):
    import fcntl
    Path(config).mkdir(parents=True, exist_ok=True)
    with open(Path(config) / 'game.lock', 'a') as stream:
        try:
            fcntl.flock(stream, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            return True
    return False


def run_game(port, config, args):
    """Keep the preparation lock across exec and the game's in-process restarts."""
    import fcntl
    Path(config).mkdir(parents=True, exist_ok=True)
    stream = open(Path(config) / 'game.lock', 'a')
    try:
        fcntl.flock(stream, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError:
        print('A game is already being prepared or running.', file=sys.stderr)
        return 1
    os.set_inheritable(stream.fileno(), True)
    os.chdir(port)
    from bbport_linux_runner import main
    try:
        return main(port, args)
    finally:
        stream.close()


class GameJob:
    """Popen(start_new_session=True) isolates a launch and its descendants."""
    def __init__(self, process):
        self.pid = process.pid

    def terminate(self):
        if self.pid is not None:
            try:
                os.killpg(self.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass

    def close(self):
        self.pid = None


class UnmanagedWindow:
    """SDL and the Linux window manager handle placement, rather than user32."""
    last = None

    def stop(self):
        pass
