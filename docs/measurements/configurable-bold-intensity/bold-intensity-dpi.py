import os, pathlib, subprocess, time
root=pathlib.Path('/tmp/bold-intensity-evidence/verified')
for family,intensity in [('static',1.25),('variable',0.75)]:
    cfg=root/f'1-{family}-16-dark-{intensity}.toml'
    done=root/f'dpi-{family}.done'; done.unlink(missing_ok=True)
    env={**os.environ,'TERMINAL_CONFIG':str(cfg),'TERMINAL_RENDERER_DIAGNOSTICS':'1','FONTCONFIG_FILE':str(root/'fonts.conf'),'BOLD_DONE_FILE':str(done)}
    proc=None
    try:
        with (root/f'dpi-{family}.log').open('w') as log:
            proc=subprocess.Popen([str(root/'terminal-candidate'),'-e','/usr/bin/python3','/tmp/bold-intensity-visual.py','probe'],env=env,stdout=subprocess.DEVNULL,stderr=log)
            time.sleep(1.2)
            for index,scale in enumerate(['1','1.25','2','1']):
                subprocess.run(['kscreen-doctor',f'output.HDMI-A-1.scale.{scale}',f'output.eDP-1.scale.{scale}'],check=True,stdout=subprocess.DEVNULL)
                time.sleep(1.5)
                assert proc.poll() is None
                js=root/'dpi-geometry.js'
                js.write_text('for (const w of workspace.windowList()) { if (w.pid === '+str(proc.pid)+') { w.setMaximize(false,false); w.frameGeometry={x:30,y:40,width:780,height:420}; w.keepAbove=true; workspace.activeWindow=w; } }')
                plugin='bold-dpi-'+str(proc.pid)
                sid=subprocess.check_output(['qdbus6','org.kde.KWin','/Scripting','org.kde.kwin.Scripting.loadScript',str(js),plugin],text=True).strip()
                subprocess.run(['qdbus6','org.kde.KWin','/Scripting/Script'+sid,'org.kde.kwin.Script.run'],check=True,stdout=subprocess.DEVNULL)
                subprocess.run(['qdbus6','org.kde.KWin','/Scripting','org.kde.kwin.Scripting.unloadScript',plugin],check=True,stdout=subprocess.DEVNULL)
                time.sleep(.5)
                raw=root/'dpi-desktop-temporary.png'
                try:
                    subprocess.run(['spectacle','-i','-f','-b','-n','-o',str(raw)],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,timeout=10)
                    factor=float(scale)
                    crop=f'{round(780*factor)}x{round(420*factor)}+{round(30*factor)}+{round(40*factor)}'
                    subprocess.run(['magick',str(raw),'-crop',crop,'+repage',str(root/f'dpi-{family}-{index}-{scale}.png')],check=True)
                finally:
                    raw.unlink(missing_ok=True)
                print(f'{family} live DPI {scale} captured',flush=True)
            done.touch()
            assert proc.wait(timeout=10)==0
        log=(root/f'dpi-{family}.log').read_text()
        for scale in ['1.25','2']: assert f'scale_factor={scale}' in log
    finally:
        subprocess.run(['kscreen-doctor','output.HDMI-A-1.scale.1','output.eDP-1.scale.1'],check=True,stdout=subprocess.DEVNULL)
        if proc is not None and proc.poll() is None:
            proc.terminate(); proc.wait(timeout=5)
