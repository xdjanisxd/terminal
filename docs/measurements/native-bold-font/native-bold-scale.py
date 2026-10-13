import os, pathlib, subprocess, time
root=pathlib.Path('/tmp/native-bold-validation')
proc=None
try:
    with open(root/'dpi-transitions.log','w') as log:
        proc=subprocess.Popen([str(root/'terminal-candidate'),'-e','/usr/bin/python3','/tmp/native-bold-visual.py','probe','ansi'],stdout=subprocess.DEVNULL,stderr=log,env={**os.environ,'TERMINAL_CONFIG':str(root/'config.toml'),'TERMINAL_RENDERER_DIAGNOSTICS':'1','NATIVE_BOLD_HOLD':'25'})
        for index,scale in enumerate(['1','1.25','2','1']):
            subprocess.run(['kscreen-doctor',f'output.HDMI-A-1.scale.{scale}',f'output.eDP-1.scale.{scale}'],check=True,stdout=subprocess.DEVNULL)
            time.sleep(3)
            assert proc.poll() is None
            subprocess.run(['spectacle','-a','-b','-n','-o',str(root/f'dpi-{index}-{scale}.png')],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
            print('captured scale',scale,flush=True)
        assert proc.wait(timeout=20)==0
finally:
    subprocess.run(['kscreen-doctor','output.HDMI-A-1.scale.1','output.eDP-1.scale.1'],check=True,stdout=subprocess.DEVNULL)
    if proc is not None and proc.poll() is None: proc.terminate();proc.wait(timeout=5)
