import json, os, pathlib, subprocess, time
root = pathlib.Path('/tmp/blocked-pty-shutdown/native-close')
root.mkdir(exist_ok=True)
binary = '/home/dellian/codes/terminal/target/release/terminal'
def wait(check, description):
    deadline = time.monotonic() + 30
    while not check():
        if time.monotonic() >= deadline: raise AssertionError(description)
        time.sleep(.01)
def dbus(*args):
    return subprocess.check_output(['qdbus6', 'org.kde.KWin', *args], text=True).strip()
for shell in ['bash', 'zsh']:
    for count in [1, 3]:
        for stage in ['early', 'ready']:
            case = root / f'{shell}-{count}-{stage}'
            case.mkdir(exist_ok=True)
            temporary = case / 'tmp'
            temporary.mkdir(exist_ok=True)
            home = case / 'home'
            home.mkdir(exist_ok=True)
            config = f'[shell]\nprogram = "{shell}"\n[shell.env]\nHOME = "{home}"\n[workspace]\n'
            for pane in range(count):
                config += f'[[workspace.tabs]]\ntitle = "Lifecycle"\nlayout = {{ kind = "pane", session = "local_shell", startup_command = "echo $$ > {case}/pid-{pane}" }}\n'
            path = case / 'workspace.toml'
            path.write_text(config)
            saved = case / 'config/terminal/workspaces.toml'
            saved.parent.mkdir(parents=True, exist_ok=True)
            saved.write_text('# preserved\nworkspaces = []\n')
            before = saved.read_bytes()
            env = os.environ | {'TMPDIR': str(temporary), 'XDG_CONFIG_HOME': str(case/'config'), 'TERMINAL_STARTUP_DIAGNOSTICS': '1', 'TERMINAL_RENDERER_DIAGNOSTICS': '1'}
            plugin = f'terminal-lifecycle-{shell}-{count}-{stage}'
            with (case/'stderr.log').open('w') as log:
                process = subprocess.Popen([binary, '--workspace', str(path)], env=env, stdout=subprocess.DEVNULL, stderr=log)
                try:
                    if stage == 'ready':
                        wait(lambda: all((case/f'pid-{i}').exists() for i in range(count)), 'shell readiness')
                    # Match only the launched PID. windowAdded also covers early close
                    # before the Wayland surface is mapped; never target by focus/title.
                    script = case/'close.js'
                    script.write_text(f'function close(w) {{ if (w.pid === {process.pid}) w.closeWindow(); }} workspace.windowAdded.connect(close); for (const w of workspace.windowList()) close(w);')
                    script_id = dbus('/Scripting', 'org.kde.kwin.Scripting.loadScript', str(script), plugin)
                    assert int(script_id) >= 0
                    start = time.monotonic()
                    dbus(f'/Scripting/Script{script_id}', 'org.kde.kwin.Script.run')
                    assert process.wait(timeout=30) == 0
                    elapsed = time.monotonic() - start
                    pids = [int(p.read_text().strip()) for p in case.glob('pid-*')]
                    assert all(not pathlib.Path(f'/proc/{pid}').exists() for pid in pids), pids
                    assert not list(temporary.iterdir()), 'integration files leaked'
                    assert saved.read_bytes() == before
                    text = (case/'stderr.log').read_text()
                    assert 'app event=close-requested' in text
                    assert text.index('stage=first-frame-presented') < text.index('stage=pty-started'), text
                    print(json.dumps({'shell': shell, 'panes': count, 'stage': stage, 'close_to_exit_ms': round(elapsed*1000, 2), 'shell_pids': pids}), flush=True)
                finally:
                    if process.poll() is None:
                        process.kill()
                        process.wait()
                    try: dbus('/Scripting', 'org.kde.kwin.Scripting.unloadScript', plugin)
                    except subprocess.CalledProcessError: pass
print('evidence:', root)
