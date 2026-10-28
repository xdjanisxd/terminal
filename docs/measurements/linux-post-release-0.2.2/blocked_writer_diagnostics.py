import os,pathlib,struct,subprocess,time
root=pathlib.Path('/tmp/terminal-v022-post-release/blocked-writer-diagnostics'); root.mkdir(exist_ok=True)
for n in ('home','data','config','cache'): (root/n).mkdir(exist_ok=True)
child=root/'child.py'
child.write_text('import os,tty,time\ntty.setraw(0)\nopen('+repr(str(root/'ready'))+',"w").write(str(os.getpid()))\nfor i in range(20000):\n os.write(1,b"\\x1b[6n")\n time.sleep(.001)\ntime.sleep(30)\n')
env=os.environ.copy(); env.update(HOME=str(root/'home'),XDG_DATA_HOME=str(root/'data'),XDG_CONFIG_HOME=str(root/'config'),XDG_CACHE_HOME=str(root/'cache'),TERMINAL_RENDERER_DIAGNOSTICS='1')
with (root/'stderr.log').open('w') as log:
 app=subprocess.Popen(['/tmp/terminal-v022-post-release/extracted/terminal','-e','/usr/bin/python3',str(child)],env=env,stdout=log,stderr=log)
 plugin='postrelease-blocked-writer-diagnostics-'+str(app.pid); sid=None
 try:
  deadline=time.monotonic()+25; found=None
  while time.monotonic()<deadline and app.poll() is None:
   try:
    mem=os.open('/proc/'+str(app.pid)+'/mem',os.O_RDONLY)
    try:
     for task in pathlib.Path('/proc',str(app.pid),'task').iterdir():
      try:
       data=(task/'syscall').read_text().split()
       if len(data)<4 or int(data[0])!=7 or int(data[2],0)!=2: continue
       raw=os.pread(mem,16,int(data[1],0)); a=struct.unpack('ihhihh',raw)
       if a[1]==1 and a[4]==4:
        found=(task.name,a,os.readlink('/proc/'+str(app.pid)+'/fd/'+str(a[3]))); break
      except (OSError,ValueError,struct.error): pass
    finally: os.close(mem)
   except OSError: pass
   if found: break
   time.sleep(.05)
  print('blocked_poll',found,flush=True)
  (root/'observation.txt').write_text(repr(found))
  subprocess.run(['spectacle','-a','-b','-n','-o',str(root/'before-close.png')],check=True)
  js=root/'close.js'; js.write_text('for (const w of workspace.windowList()) { if (w.pid === '+str(app.pid)+') w.closeWindow(); }')
  sid=subprocess.check_output(['qdbus6','org.kde.KWin','/Scripting','org.kde.kwin.Scripting.loadScript',str(js),plugin],text=True).strip()
  start=time.monotonic(); subprocess.run(['qdbus6','org.kde.KWin','/Scripting/Script'+sid,'org.kde.kwin.Script.run'],check=True,stdout=subprocess.DEVNULL)
  rc=app.wait(timeout=15); print('exit',rc,'seconds',round(time.monotonic()-start,3),flush=True)
  assert rc==0
 finally:
  if app.poll() is None: app.kill(); app.wait()
  if sid: subprocess.run(['qdbus6','org.kde.KWin','/Scripting','org.kde.kwin.Scripting.unloadScript',plugin],stdout=subprocess.DEVNULL)
