#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-or-later
"""Platform-neutral entry point; the legacy Windows entry point stays compatible."""
import json
import os
from pathlib import Path
import sys


def main():
    if getattr(sys, 'frozen', False):
        sys.path.insert(0, str(Path(sys._MEIPASS) / 'scripts'))
        if sys.platform.startswith('linux') and not os.environ.get('FONTCONFIG_FILE'):
            config = str(Path(sys._MEIPASS) / 'assets/fonts/fonts.conf')
            os.environ['FONTCONFIG_FILE'] = config
            os.environ['BB_BUNDLED_FONTCONFIG'] = config
    import bbport_platform
    bbport_platform.configure_portable()
    if getattr(sys, 'frozen', False) and not any(
            flag in sys.argv for flag in ('--check-runtime', '--smoke-test', '--run', '--script')):
        bbport_platform.migrate_user_data(Path(sys.executable).resolve().parent)
    import bbport_launcher_win as app
    if '--check-runtime' in sys.argv:
        app.attach_stdio()
        import tkinter
        import PIL.Image
        import ssl
        import bbport_theme
        # Tcl initialization also checks that the frozen bundle carries Tcl's library.
        tkinter.Tcl().eval('info patchlevel')
        assert bbport_theme.ASSETS.is_dir(), 'launcher assets missing'
        import patches
        assert patches.BUILTIN_XML.is_file(), 'embedded patch XML missing'
        print(json.dumps({'platform': app.host.family(), 'tk': tkinter.TkVersion,
                          'assets': str(bbport_theme.ASSETS), 'ssl': ssl.OPENSSL_VERSION}))
        return 0
    if '--smoke-test' in sys.argv:
        app.attach_stdio()
        return smoke_test(app)
    app.saves.OUTSIDE = app.saves.outside_dir()
    app.main()
    return 0


def smoke_test(app):
    """Build every page on an isolated profile; needs a display (Xvfb in Linux CI)."""
    import tempfile
    import traceback
    import tkinter as tk
    from tkinter import ttk, filedialog, messagebox

    with tempfile.TemporaryDirectory(prefix='bbport-ui-') as folder:
        os.environ['BB_CONFIG'] = str(Path(folder) / 'bbport.ini')
        os.environ['BB_DATA_DIR'] = folder
        app.DATA_DIR = app.CONFIG_DIR = Path(folder)
        app.CONFIG_FILE = Path(folder) / 'settings.json'
        app.CONFIG_FILE.write_text(json.dumps({'check_updates': False, 'setup_done': True}), encoding='utf-8')
        root = tk.Tk()
        errors = []
        def failed(*args):
            errors.append(''.join(traceback.format_exception(*args)))
        root.report_callback_exception = failed
        # A smoke test is not a new player's first run, and must not open modal guides.
        app.Launcher.maybe_open_setup = lambda self: None
        app.Launcher.maybe_show_news = lambda self: None
        messagebox.showerror = lambda *args, **kwargs: errors.append(str(args))
        messagebox.showinfo = lambda *args, **kwargs: errors.append(str(args))
        messagebox.showwarning = lambda *args, **kwargs: errors.append(str(args))

        def close_window(window):
            destroyed = []
            window.root.bind('<Destroy>', lambda event: destroyed.append(True)
                             if event.widget is window.root else None, add='+')
            window.root.tk.call(window.root.protocol('WM_DELETE_WINDOW'))
            if not destroyed:
                window.root.destroy()
                raise AssertionError('WM_DELETE_WINDOW did not close the idle launcher')
        window = app.Launcher(root, tk, ttk, filedialog, messagebox)
        try:
            for name, _title in app.NAV_SIMPLE + app.NAV_EXPERT:
                window.show(name)
                root.update()
            root.update_idletasks()
        finally:
            window.pad_stop()
            frame = getattr(window, 'custom_frame', None)
            if frame:
                frame.uninstall()
            for job in root.tk.splitlist(root.tk.call('after', 'info')):
                root.after_cancel(job)
            close_window(window)
        if errors:
            raise RuntimeError('\n'.join(errors))
        print('PASS: all launcher pages')
        # Exercise the real startup path too: it performs frozen-only work that
        # constructing Launcher directly cannot cover (e.g. update cleanup).
        original = app.Launcher
        startup = []
        class StartupWindow(original):
            def __init__(self, root, *args):
                super().__init__(root, *args)
                startup.append(self)
                root.after(700, root.quit)
            def callback_error(self, *args):
                failed(*args)
                self.root.quit()
        saved = app.Launcher, app.one_launcher, app.bring_to_front_on_request, sys.argv
        app.Launcher = StartupWindow
        app.one_launcher = lambda: True
        app.bring_to_front_on_request = lambda *args: None
        sys.argv = [sys.argv[0]]
        try:
            app.main()
            assert startup, 'normal entry point did not create the launcher'
        finally:
            for window in startup:
                window.pad_stop()
                if window.custom_frame:
                    window.custom_frame.uninstall()
                for job in window.root.tk.splitlist(window.root.tk.call('after', 'info')):
                    window.root.after_cancel(job)
                close_window(window)
            app.Launcher, app.one_launcher, app.bring_to_front_on_request, sys.argv = saved
        if errors:
            raise RuntimeError('\n'.join(errors))
        print('PASS: normal launcher entry point and WM_DELETE_WINDOW')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
