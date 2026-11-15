#!/usr/bin/env python3
"""One-time migration aid: preserve the reference UI, not a runtime dependency.

Exports widget options/geometry, images, style commands and callback provenance.
The Rust launcher must implement each exported action before replacing the reference.
Never export a user's real profile or online credentials.
"""
import argparse
import base64
import inspect
import json
import os
from pathlib import Path
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'launcher'))


def screenshot(root, path):
    import ctypes
    from PIL import Image
    class XImage(ctypes.Structure):
        _fields_=[('width',ctypes.c_int),('height',ctypes.c_int),('xoffset',ctypes.c_int),('format',ctypes.c_int),('data',ctypes.c_void_p)]
    lib=ctypes.CDLL('libtcl9tk9.2.so')
    lib.XGetImage.argtypes=[ctypes.c_void_p,ctypes.c_ulong,ctypes.c_int,ctypes.c_int,ctypes.c_uint,ctypes.c_uint,ctypes.c_ulong,ctypes.c_int]
    lib.XGetImage.restype=ctypes.POINTER(XImage)
    lib.XDestroyImage.argtypes=[ctypes.POINTER(XImage)]
    w,h=root.winfo_width(),root.winfo_height()
    image=lib.XGetImage(None,root.winfo_id(),0,0,w,h,ctypes.c_ulong(-1),2)
    assert image
    try:
        data=ctypes.string_at(image.contents.data,w*h*4)
        path.parent.mkdir(parents=True,exist_ok=True)
        Image.frombytes('RGBA',(w,h),data,'raw','BGRA').convert('RGB').save(path)
    finally:lib.XDestroyImage(image)

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--language', default='ru')
    parser.add_argument('--profile', type=Path)
    parser.add_argument('--screenshots', type=Path)
    args = parser.parse_args()
    output = args.output
    output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='bb-rust-ui-export-') as profile:
        if args.profile:
            assert args.profile.resolve().is_relative_to(ROOT / 'out'), 'Use an isolated profile under out/'
            profile = str(args.profile.resolve())
            Path(profile).mkdir(parents=True, exist_ok=True)
        os.environ.update(BB_DATA_DIR=profile, BB_PORT_DIR=str(ROOT))
        (Path(profile) / 'settings.json').write_text(json.dumps({
            'ui_language': args.language, 'check_updates': False, 'setup_done': True,
            'ui_mode': 'expert', 'online': False, 'discord_status': False,
        }))
        import tkinter as tk
        from tkinter import ttk, filedialog, messagebox
        import bbport_launcher_win as app
        app.LANG = args.language
        app.APP_DEFAULTS['game_dir'] = '/reference/Game'
        for name in ('maybe_open_setup', 'maybe_show_news', 'detect_gpu', 'check_update', 'poll_trophies', 'temporary_warning', 'drain_output'):
            setattr(app.Launcher, name, lambda *a, **kw: None)
        root = tk.Tk()
        real = root.tk
        callbacks, style_commands, font_commands = {}, [], []
        def value(obj):
            if isinstance(obj, tk.Variable): return str(obj)
            if isinstance(obj, tk.Misc): return str(obj)
            if isinstance(obj, Path): return str(obj)
            if isinstance(obj, (tuple, list)): return [value(v) for v in obj]
            if isinstance(obj, dict): return {str(k): value(v) for k, v in obj.items()}
            if isinstance(obj, bytes): return {'binary': base64.b64encode(obj).decode()}
            if obj is None or isinstance(obj, (str, int, float, bool)): return obj
            return str(obj)

        def provenance(callback):
            call_wrapper = getattr(callback, '__self__', None)
            func = getattr(call_wrapper, 'func', callback)
            result = {'name': getattr(func, '__qualname__', str(func)), 'arguments': {}}
            bound = getattr(func, '__self__', None)
            if isinstance(bound, tk.Misc): result['widget'] = str(bound)
            setter = getattr(func, '_setit__var', None)
            if setter is not None:
                result.update(set_variable=str(setter), set_value=value(getattr(func, '_setit__value')))
            code = getattr(func, '__code__', None)
            if code:
                try: result['source'] = Path(code.co_filename).read_text().splitlines()[code.co_firstlineno-1].strip()
                except (OSError,IndexError): pass
                result.update(file=str(Path(code.co_filename).relative_to(ROOT)) if Path(code.co_filename).is_relative_to(ROOT) else Path(code.co_filename).name,
                              line=code.co_firstlineno)
                closure = getattr(func, '__closure__', None) or ()
                for name, cell in zip(code.co_freevars, closure):
                    try:
                        v = cell.cell_contents
                        if name != 'self' and v is not window_holder[0]: result['arguments'][name] = value(v)
                    except ValueError: pass
                if getattr(func, '__defaults__', None): result['defaults'] = value(func.__defaults__)
            return result
        window_holder = [None]
        class Recording:
            def __getattr__(self, name): return getattr(real, name)
            def call(self, *command):
                flat = command[0] if len(command) == 1 and isinstance(command[0], (list, tuple)) else command
                if flat and flat[0] == 'ttk::style' and len(flat) > 2:
                    style_commands.append(value(flat))
                elif flat and flat[0] == 'font' and len(flat) > 2:
                    font_commands.append(value(flat))
                return real.call(*command)
            def createcommand(self, name, callback):
                callbacks[name] = provenance(callback)
                return real.createcommand(name, callback)
        root.tk = Recording()
        window = app.Launcher(root, tk, ttk, filedialog, messagebox)
        window_holder[0] = window
        root.update()
        # Lay out every page once, while keeping the Play page as the initial view.
        for name in window.pages:
            window.show_page(name)
            root.update()
            if args.screenshots:
                screenshot(root, args.screenshots / (name + '.png'))
        window.show_page('play')
        root.update()
        image_dir = output / 'images'
        image_dir.mkdir(exist_ok=True)
        images = []
        for name in real.splitlist(real.call('image', 'names')):
            if str(name).startswith('pyimage') and real.call('image', 'type', name) == 'photo':
                data = real.call(name, 'data', '-format', 'png')
                if isinstance(data, str): data = data.encode('latin1')
                (image_dir / (str(name) + '.png')).write_bytes(data)
                images.append({'name': str(name), 'file': 'images/' + str(name) + '.png'})
        classes = {'Frame': 'frame', 'Label': 'label', 'Canvas': 'canvas', 'Text': 'text', 'Button': 'button',
                   'Entry': 'entry', 'Listbox': 'listbox', 'Scrollbar': 'scrollbar', 'Checkbutton': 'checkbutton',
                   'Radiobutton': 'radiobutton', 'Scale': 'scale', 'Spinbox': 'spinbox', 'Labelframe': 'labelframe', 'Panedwindow': 'panedwindow',
                   'Menu': 'menu', 'Menubutton': 'menubutton', 'TMenubutton': 'ttk::menubutton', 'TFrame': 'ttk::frame', 'TLabel': 'ttk::label', 'TButton': 'ttk::button', 'TEntry': 'ttk::entry',
                   'TCombobox': 'ttk::combobox', 'TCheckbutton': 'ttk::checkbutton', 'TRadiobutton': 'ttk::radiobutton',
                   'TScrollbar': 'ttk::scrollbar', 'TScale': 'ttk::scale', 'TSpinbox': 'ttk::spinbox',
                   'TSeparator': 'ttk::separator', 'TProgressbar': 'ttk::progressbar', 'TNotebook': 'ttk::notebook'}
        widgets, canvas_items, bindings, geometry, menus = [], [], [], [], []
        def walk(widget):
            for child in widget.winfo_children():
                if isinstance(child, tk.Toplevel): continue
                kind = child.winfo_class()
                if kind not in classes: raise ValueError('Unmapped widget class: ' + kind)
                options = {key: value(option[-1]) for key, option in child.configure().items() if len(option) == 5}
                widgets.append({'path': str(child), 'command': classes[kind], 'options': options,
                                'propagate': {'pack': bool(child.pack_propagate()), 'grid': bool(child.grid_propagate())},
                                'states': list(child.state()) if isinstance(child, ttk.Widget) else []})
                if isinstance(child, ttk.Combobox):
                    widgets[-1]['contents'] = {'text': child.get(), 'current': child.current()}
                elif isinstance(child, (tk.Entry, ttk.Entry)) and not str(child.cget('textvariable')):
                    widgets[-1]['contents'] = {'text': child.get()}
                elif isinstance(child, tk.Text):
                    widgets[-1]['contents'] = {'text': child.get('1.0','end-1c')}
                elif isinstance(child, tk.Listbox):
                    widgets[-1]['contents'] = {'items': list(child.get(0,'end'))}
                if isinstance(child, tk.Menu):
                    end = child.index('end')
                    for index in range((end + 1) if end is not None else 0):
                        options = {k:value(v[-1]) for k,v in child.entryconfigure(index).items()}
                        menus.append({'path':str(child), 'kind':child.type(index), 'options':options})
                if isinstance(child, tk.Canvas):
                    for item in child.find_all():
                        options = {k: value(v[-1]) for k, v in child.itemconfigure(item).items()}
                        canvas_items.append({'canvas': str(child), 'id': item, 'kind': child.type(item),
                                             'coords': child.coords(item), 'options': options})
                for sequence in child.bind():
                    script = child.bind(sequence)
                    if script: bindings.append([str(child), sequence, script])
                walk(child)
        walk(root)
        # Geometry manager order matters for pixel-for-pixel pack layouts.
        all_widgets = [root] + [root.nametowidget(w['path']) for w in widgets]
        for parent in all_widgets:
            for child in parent.pack_slaves(): geometry.append(['pack', str(child), value(child.pack_info())])
            for child in parent.grid_slaves(): geometry.append(['grid', str(child), value(child.grid_info())])
            for child in parent.place_slaves(): geometry.append(['place', str(child), value(child.place_info())])
            if parent.grid_size() != (0, 0):
                columns, rows = parent.grid_size()
                for axis, size in (('column', columns), ('row', rows)):
                    for index in range(size):
                        config = real.call('grid', axis + 'configure', str(parent), index)
                        geometry.append(['grid-' + axis, str(parent), index, value(config)])
        for sequence in real.splitlist(real.call('bind', 'all')):
            bindings.append(['all', sequence, real.call('bind', 'all', sequence)])
        variables = {key: {'name': str(var), 'store': var.store, 'value': value(var.get())}
                     for key, var in window.vars.items()}
        traces = {str(var): value(real.call('trace', 'info', 'variable', str(var))) for var in window.vars.values()}
        font_definitions = []
        for name in real.splitlist(real.call('font', 'names')):
            options = real.splitlist(real.call('font', 'configure', name))
            font_definitions.append({'name': str(name), 'options': {str(options[i]): value(options[i+1]) for i in range(0,len(options),2)}})
        for sequence in root.bind():
            script = root.bind(sequence)
            if script: bindings.append(['.', sequence, script])
        manifest = {
            'format': 1, 'language': args.language, 'reference_version': app.VERSION_TEXT,
            'geometry': root.geometry().split('+')[0], 'dpi': window.dpi,
            'pages': {k: str(v) for k, v in window.pages.items()},
            'navigation': value(app.NAV_SIMPLE + app.NAV_EXPERT),
            'nav_widgets': {key: {'canvas': str(canvas), 'items':value(items)} for key,(canvas,items) in window.nav.items()},
            'nav_pills': value(window.nav_pills),
            'widgets': widgets, 'canvas_items': canvas_items, 'layout': geometry,
            'images': images, 'fonts': font_definitions, 'styles': style_commands,
            'menus':menus, 'bindings': bindings, 'callbacks': callbacks, 'variables': variables, 'traces': traces,
            'tcl_variables': {str(name): value(real.call('set', name)) for name in real.splitlist(real.call('info','globals','PY_VAR*'))},
            'references': {key:value(obj) for key,obj in window.__dict__.items() if isinstance(obj,(tk.Misc,tk.Variable))},
            'app_defaults': value(app.APP_DEFAULTS), 'ini_defaults': app.INI_DEFAULTS,
        }
        text = json.dumps(manifest, ensure_ascii=False, indent=2)
        text = text.replace(profile, '@DATA@').replace(str(ROOT), '@INSTALL@')
        (output / 'ui.json').write_text(text)
        print(json.dumps({'widgets':len(widgets),'images':len(images),'callbacks':len(callbacks),'pages':len(window.pages)}))
        for job in real.splitlist(real.call('after', 'info')): root.after_cancel(job)
        window.pad_stop()
        root.destroy()


if __name__ == '__main__': main()
