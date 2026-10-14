import os, pathlib, subprocess, sys, time, json
ROOT = pathlib.Path('/tmp/bold-intensity-evidence/verified')
ROOT.mkdir(exist_ok=True)
if len(sys.argv) > 1 and sys.argv[1] == 'probe':
    sample = 'Ag0@8 BDPQ counters | é e\u0301 | ┌─┬─┐ │ └─┴─┘ | 日本語 \ue0b0 \uf120'
    text = '\x1b[?25l\x1b[2J\x1b[HNative weight: regular / Bold / reset\r\n\r\n'
    text += sample+'\r\n\x1b[1m'+sample+'\r\n\x1b[22m'+sample+'\r\n\r\n'
    text += '\x1b[31mRED \x1b[32mGREEN \x1b[34mBLUE\x1b[0m\r\n\x1b[1;31mRED \x1b[32mGREEN \x1b[34mBLUE\x1b[0m\r\n'
    text += '\x1b[38;5;196m256 red \x1b[1mBold red\x1b[22m \x1b[38;2;0;160;200mRGB cyan \x1b[1mBold cyan\x1b[0m\r\n'
    os.write(1, text.encode())
    for _ in range(300):
        if pathlib.Path(os.environ['BOLD_DONE_FILE']).exists(): break
        time.sleep(0.1)
    sys.exit()

assert os.environ.get('XDG_SESSION_TYPE') == 'wayland'
fonts = ROOT/'fonts.conf'
fonts.write_text('<?xml version="1.0"?><!DOCTYPE fontconfig SYSTEM "urn:fontconfig:fonts.dtd"><fontconfig><include>/etc/fonts/fonts.conf</include><dir>/home/dellian/codes/terminal/crates/renderer/tests/fixtures</dir></fontconfig>')

def geometry(pid):
    path = ROOT/'geometry.js'
    path.write_text('for (const w of workspace.windowList()) { if (w.pid === '+str(pid)+') { w.setMaximize(false, false); w.frameGeometry = { x: 30, y: 40, width: 780, height: 420 }; w.keepAbove = true; workspace.activeWindow = w; } }')
    plugin = 'bold-intensity-capture-'+str(pid)
    out = subprocess.check_output(['qdbus6','org.kde.KWin','/Scripting','org.kde.kwin.Scripting.loadScript',str(path),plugin],text=True).strip()
    try:
        subprocess.run(['qdbus6','org.kde.KWin','/Scripting/Script'+out,'org.kde.kwin.Script.run'],check=True,stdout=subprocess.DEVNULL)
    finally:
        subprocess.run(['qdbus6','org.kde.KWin','/Scripting','org.kde.kwin.Scripting.unloadScript',plugin],check=True,stdout=subprocess.DEVNULL)

def capture(scale, family, size, theme, intensity, baseline=False):
    label = f'{scale}-{family}-{size}-{theme}-'+('baseline' if baseline else str(intensity))
    if (ROOT/(label+'.png')).exists():
        print(label, 'already captured', flush=True)
        return
    cfg = ROOT/(label+'.toml')
    names = {'static':'JetBrainsMono Nerd Font Mono','variable':'JetBrains Mono','fallback':'Noto Mono'}
    fg,bg = ('#e6e6e6','#000000') if theme=='dark' else ('#202020','#eeeeee')
    cfg.write_text(f'[font]\nfamily = "{names[family]}"\nsize = {size}\n'+('' if intensity is None else f'bold_intensity = {intensity}\n')+f'[theme]\nforeground = "{fg}"\nbackground = "{bg}"\n')
    binary = ROOT/('terminal-baseline' if baseline else 'terminal-candidate')
    done = ROOT/(label+'.done')
    done.unlink(missing_ok=True)
    env = {**os.environ,'TERMINAL_CONFIG':str(cfg),'TERMINAL_RENDERER_DIAGNOSTICS':'1','FONTCONFIG_FILE':str(fonts),'BOLD_DONE_FILE':str(done)}
    with (ROOT/(label+'.log')).open('w') as log:
        proc = subprocess.Popen([str(binary),'-e','/usr/bin/python3',__file__,'probe'],stdout=subprocess.DEVNULL,stderr=log,env=env)
        try:
            time.sleep(1)
            assert proc.poll() is None, label
            geometry(proc.pid)
            time.sleep(1.4)
            raw = ROOT/'desktop-temporary.png'
            try:
                active=os.environ.get('BOLD_CAPTURE_ACTIVE') == '1'
                if active: geometry(proc.pid)
                subprocess.run(['spectacle','-i','-a' if active else '-f','-b','-n','-o',str(raw)],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,timeout=10)
                factor=float(scale)
                crop=f'{round(780*factor)}x{round(420*factor)}+{round(30*factor)}+{round(40*factor)}'
                if active:
                    subprocess.run(['magick',str(raw),str(ROOT/(label+'.png'))],check=True)
                else:
                    subprocess.run(['magick',str(raw),'-crop',crop,'+repage',str(ROOT/(label+'.png'))],check=True)
            finally:
                raw.unlink(missing_ok=True)
            done.touch()
            assert proc.wait(timeout=15)==0, label
        finally:
            if proc.poll() is None:
                proc.terminate(); proc.wait(timeout=5)
    contents=(ROOT/(label+'.log')).read_text()
    assert 'backend=Wayland' in contents,label
    assert f'scale_factor={scale}' in contents,label
    assert 'panic' not in contents,label
    print(label, 'captured', flush=True)

try:
    if len(sys.argv) > 1 and sys.argv[1] == 'one':
        scale,family,size,theme,intensity=sys.argv[2:]
        subprocess.run(['kscreen-doctor',f'output.HDMI-A-1.scale.{scale}',f'output.eDP-1.scale.{scale}'],check=True,stdout=subprocess.DEVNULL)
        time.sleep(1.5)
        capture(scale,family,int(size),theme,float(intensity))
        sys.exit()
    for scale in ['1','1.25','2']:
        subprocess.run(['kscreen-doctor',f'output.HDMI-A-1.scale.{scale}',f'output.eDP-1.scale.{scale}'],check=True,stdout=subprocess.DEVNULL)
        time.sleep(1.5)
        capture(scale,'static',16,'dark',None,True)
        capture(scale,'static',16,'dark',None)
        for family in ['static','variable']:
            for intensity in [0.75,1.0,1.25]: capture(scale,family,16,'dark',intensity)
        if scale in ['1','2']:
            size=12 if scale=='1' else 20
            for family in ['static','variable']:
                for intensity in [0.75,1.0,1.25]: capture(scale,family,size,'light',intensity)
    for intensity in [0.75,1.0,1.25]: capture('2','fallback',16,'dark',intensity)
finally:
    subprocess.run(['kscreen-doctor','output.HDMI-A-1.scale.1','output.eDP-1.scale.1'],check=True,stdout=subprocess.DEVNULL)
