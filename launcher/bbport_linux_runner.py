# SPDX-License-Identifier: GPL-2.0-or-later
"""Native Linux launch preparation, shared by source and single-file packages.

The installed launcher runs these steps in its embedded Python: no shell wrapper
or system Python is involved. The native runtime remains a separate directory.
"""
import argparse
import contextlib
import io
import json
import os
from pathlib import Path
import runpy
import subprocess
import sys


def resource_root():
    return Path(getattr(sys, '_MEIPASS', Path(__file__).resolve().parent.parent))


def script(name, *args, capture=False, optional=False, timeout=None):
    # These local preparation scripts have no untrusted stdin and run sequentially.
    path = resource_root() / 'scripts' / name
    previous = sys.argv
    output = io.StringIO()
    status = 0
    try:
        sys.argv = [str(path), *map(str, args)]
        with contextlib.redirect_stdout(output) if capture else contextlib.nullcontext():
            try:
                runpy.run_path(str(path), run_name='__main__')
            except SystemExit as stop:
                status = stop.code or 0
    finally:
        sys.argv = previous
    if status and not optional:
        raise SystemExit(status)
    return (output.getvalue() if not status else None) if capture else (not status if optional else None)


def prepare(port, args=()):
    import linux_start as start
    import patches
    import run
    run.run_script = script
    # `run` is shared with the Windows preparation steps; all files live in the bundle.
    run.PORT = resource_root()
    env = os.environ
    port = Path(port).resolve()
    args = list(args)
    if args[:1] == ['--software']:
        args.pop(0)
        if not env.get('VK_DRIVER_FILES'):
            candidates = list(Path('/usr/share/vulkan/icd.d').glob('lvp_icd*.json'))
            candidates += list(Path('/run/opengl-driver/share/vulkan/icd.d').glob('lvp_icd*.json'))
            if not candidates:
                raise FileNotFoundError('Lavapipe not found; set VK_DRIVER_FILES.')
            env['VK_DRIVER_FILES'] = str(candidates[0])
        env['VK_LOADER_LAYERS_DISABLE'] = '~implicit~'
    data = Path(env.get('BB_DATA_DIR') or Path.home() / '.local/share/Bloodborne').resolve()
    out = data / 'out'
    out.mkdir(parents=True, exist_ok=True)
    config = env.setdefault('BB_CONFIG', str(data / 'bbport.ini'))
    user = env.setdefault('BB_USER_DIR', str(data / 'user'))
    env.setdefault('BB_LOCAL_CHALICE_CATALOG', str(resource_root() / 'data/local_chalice/catalog.bin'))
    if not env.get('BB_FSR411_DIR') and (data / 'fsr4_411').is_dir():
        env['BB_FSR411_DIR'] = str(data / 'fsr4_411')
    game = Path(env.get('BB_GAME_DIR') or port / 'Game').resolve()
    if not (game / 'eboot.bin').is_file():
        raise FileNotFoundError(f'No eboot.bin in {game}; select the game folder in the launcher.')
    results = start.Results()
    start.attempt('before-view', start.before_view,
                  argparse.Namespace(game=str(game), out=str(out), config=config, user=user), results)
    layers = [line[6:] for line in results.lines if line.startswith('layer ')]
    groups = next((line[7:] for line in results.lines if line.startswith('groups ')), 'unknown')
    extra = [env.get('BB_PATCHES', '')]
    extra += [line[6:] for line in results.lines if line.startswith('patch ')]
    mod_args = ['--out', out, '--mods-dir', env.get('BB_MODS_DIR', str(data / 'mods')),
                '--config', env.get('BB_MODS_CONFIG', str(data / 'mods.json')),
                '--enabled', env.get('BB_MODS_ENABLED', '1')]
    layer_args = [value for layer in layers for value in ('--layer', layer)]
    try:
        view = script('mods.py', game, *mod_args, *layer_args, capture=True)
    except (OSError, SystemExit):
        env.update(BB_QUIT_TO_DESKTOP='0', BB_PC_MENU='0', BB_REBIRTH='0')
        view = script('mods.py', game, *mod_args, capture=True)
    original = game
    game = Path(view.strip().splitlines()[-1])
    # Return our own overlay for cleanup even if a later preparation step fails.
    overlay = game if game.name.startswith('mod-game-') and game != original else None
    try:
        for name in ('prepare.py', 'link_libc.py', 'link_modules.py'):
            script(name, game, '--out', out)
        compat = json.loads((out / 'link.json').read_text()).get('cpu_compat', {}).get('enabled')
        if compat:
            env['BB_CPU_COMPAT'] = '1'
        else:
            env.pop('BB_CPU_COMPAT', None)
        script('content_profile.py', game, '--out', out, '--sku', env.get('BB_CONTENT_SKU', 'full'))
        if env.get('BB_AUTO_RENDER_RES') == '1':
            for key in ('BB_RENDER_RES', 'BB_OUTPUT_RES', 'BB_AUTO_RENDER_RES'):
                env.pop(key, None)
        fps = env.get('BB_FPS', 'uncap')
        if groups != 'unknown' and 'fps' not in groups.split(','):
            fps = '30'
        runtime = Path(env.get('BB_RUNTIME_DIR') or port / 'game_runtime')
        probe = Path(env.get('BB_PROBE') or runtime / 'bb-probe')
        caps = probe.parent / 'bb-gpu-capabilities'
        settings = patches.read_settings(config)
        scaled = patches.scaled_sizes(settings) if not env.get('BB_RENDER_RES') else None
        output = f'{scaled[1][0]}x{scaled[1][1]}' if scaled else ''
        live = '0'
        if scaled:
            live = env.get('BB_LIVE_RES') or settings.get('live_resolution', '0')
            if live == 'auto':
                if settings.get('effect_dof', '1') == '1' and (scaled[1][0] > 1920 or scaled[1][1] > 1080):
                    live = '0'
                else:
                    try:
                        live = subprocess.check_output([str(caps), '--live-resolution'], text=True).strip()
                    except (OSError, subprocess.SubprocessError):
                        live = '0'
            live = '1' if live == '1' else '0'
        params = argparse.Namespace(config=config, live=live, output=output, groups=groups, caps=str(caps))
        start.attempt('ui-res', start.ui_res, params, results)
        if live != '1' and scaled:
            env.update(BB_RENDER_RES=f'{scaled[0][0]}x{scaled[0][1]}', BB_OUTPUT_RES=output, BB_AUTO_RENDER_RES='1')
            env.setdefault('BB_DMEM_MB', '9152')
        start.attempt('low-vram', start.low_vram, params, results)
        script('patches.py', '--out', out, '--fps', fps, '--extra', ';'.join(filter(None, extra)),
               '--settings', config, '--game-dir', game, '--render-res', env.get('BB_RENDER_RES', ''),
               '--output-res', env.get('BB_OUTPUT_RES', ''),
               '--patches-dir', env.get('BB_PATCHES_DIR', str(data / 'patches')),
               '--patches-config', env.get('BB_PATCHES_CONFIG', str(data / 'patches.json')))
        if env.get('BB_AUTO_VBLANK') == '1' or not env.get('BB_VBLANK_HZ'):
            env.update(BB_VBLANK_HZ={'uncap': '0', '90': '90'}.get(fps, '60'), BB_AUTO_VBLANK='1')
        command = [str(probe), str(out / 'boot-linked.bin'), '--content-profile', str(out / 'content.bin'),
                   '--patches', str(out / 'patches.bin'), '--app0', str(game), '--user', user,
                   '--timeout', env.get('BB_TIMEOUT', '0'), *args]
        return command, overlay, original
    except BaseException:
        cleanup(overlay, original)
        raise


def cleanup(overlay, original):
    if overlay and overlay.is_dir():
        from mods import remove_overlay
        remove_overlay(overlay, original)


def main(port, args=()):
    while True:
        command, overlay, original = prepare(port, args)
        try:
            env = os.environ.copy()
            env['BB_LAUNCHER_MANAGED'] = '1'
            private = str(Path(command[0]).resolve().parent / 'lib')
            env['LD_LIBRARY_PATH'] = private + (':' + env['LD_LIBRARY_PATH'] if env.get('LD_LIBRARY_PATH') else '')
            # Keep the embedded Python/temporary bundle alive while native code uses its assets.
            status = subprocess.Popen(command, env=env).wait()
        finally:
            cleanup(overlay, original)
        if status != 85:
            return status
