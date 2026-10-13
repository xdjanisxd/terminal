import signal
import os, pathlib, subprocess, sys, time, pty, select, fcntl, termios, struct
ROOT=pathlib.Path('/tmp/native-bold-validation')
TEXT='Ag0@ Bold counters | é e\u0301 日本語 \ue0b0 \uf120\n'
if len(sys.argv)>1 and sys.argv[1]=='cleanup':
    for entry in pathlib.Path('/proc').iterdir():
        if not entry.name.isdigit(): continue
        try: args=(entry/'cmdline').read_bytes().split(b'\0')
        except (OSError,PermissionError): continue
        if args and (args[0]==b'/tmp/native-bold-validation/terminal-candidate' or (len(args)>2 and args[1]==os.fsencode(__file__) and args[2]==b'probe')):
            os.kill(int(entry.name),signal.SIGTERM)
    sys.exit()
if len(sys.argv)>1 and sys.argv[1]=='probe':
    case=sys.argv[2]
    if case=='ansi':
        os.write(1, ('\x1b[2J\x1b[H'+os.environ.get('NATIVE_BOLD_FAMILY','JetBrainsMono Nerd Font Mono')+': regular / Bold / reset\r\n\r\n'+TEXT.replace('\n','\r\n')+'\x1b[1m'+TEXT.replace('\n','\r\n')+'\x1b[22m'+TEXT.replace('\n','\r\n')+'\r\n\x1b[31mRED \x1b[32mGREEN \x1b[34mBLUE \x1b[36mCYAN\x1b[0m\r\n\x1b[1;31mRED \x1b[32mGREEN \x1b[34mBLUE \x1b[36mCYAN\x1b[0m\r\n\x1b[38;5;196m256 red \x1b[1mBold red\x1b[22m\x1b[38;2;0;200;255m RGB cyan \x1b[1mBold cyan\x1b[0m\r\n').encode())
        time.sleep(float(os.environ.get("NATIVE_BOLD_HOLD", "7"))); sys.exit()
    rows,cols=struct.unpack('HHHH',fcntl.ioctl(0,termios.TIOCGWINSZ,b'\0'*8))[:2]
    pid,fd=pty.fork()
    if pid==0:
        if case=='zsh': os.execv('/usr/bin/zsh',['zsh','-f'])
        if case=='mtr': os.execv('/usr/bin/mtr',['mtr','-n','-i','1','-m','3','127.0.0.1'])
        os.execv('/usr/bin/'+case,[case,'-u','NONE','-n','-c','set termguicolors','-c','highlight NativeBold cterm=bold ctermfg=2 gui=bold guifg=#00cc88','-c','syntax match NativeBold /Bold.*/',str(ROOT/'sample.txt')])
    fcntl.ioctl(fd,termios.TIOCSWINSZ,struct.pack('HHHH',rows,cols,0,0))
    start=time.monotonic(); sent=False; quitting=False; transcript=bytearray()
    while time.monotonic()-start<12:
        elapsed=time.monotonic()-start
        if case=='zsh' and elapsed>0.5 and not sent:
            os.write(fd,b"source /usr/share/zsh-syntax-highlighting/zsh-syntax-highlighting.zsh; ZSH_HIGHLIGHT_STYLES[command]='fg=green,bold'; ZSH_HIGHLIGHT_STYLES[unknown-token]='fg=red,bold'; PROMPT='Native Bold %> '\n")
            time.sleep(.15);os.write(fd,b'echo "syntax highlighting" && invalid-command');sent=True
        if case=='zsh' and 5.5<elapsed<5.6: os.write(fd,b'\x03')
        if elapsed>6 and not quitting:
            os.write(fd,b'exit\n' if case=='zsh' else b'q' if case=='mtr' else b'\x1b:qa!\r');quitting=True
        if select.select([fd],[],[],.05)[0]:
            try: data=os.read(fd,65536)
            except OSError: break
            if not data: break
            transcript.extend(data);os.write(1,data)
    ROOT.joinpath(case+'.ansi').write_bytes(transcript)
    os.close(fd)
    os.waitpid(pid,0)
    os.write(1,b'\x1b[0m\r\nReturned successfully from application\r\n');time.sleep(1)
else:
    assert os.environ.get('XDG_SESSION_TYPE')=='wayland'
    ROOT.mkdir(exist_ok=True)
    (ROOT/'sample.txt').write_text('Regular '+TEXT+'Bold '+TEXT+'Regular reset '+TEXT)
    cfg=ROOT/'config.toml'; cfg.write_text('[font]\nfamily = "'+('Noto Mono' if len(sys.argv)>2 and sys.argv[2]=='fallback' else 'JetBrainsMono Nerd Font Mono')+'"\nsize = 16\n[window]\nopacity = 1.0\nblur = false\n')
    binary=sys.argv[1] if len(sys.argv)>1 else '/home/dellian/codes/terminal/target/release/terminal'
    label=sys.argv[2] if len(sys.argv)>2 else 'candidate'
    for case in (sys.argv[3:] or ['ansi','zsh','mtr','vim','nvim']):
        with open(ROOT/f'{label}-{case}.log','w') as log:
            proc=subprocess.Popen([binary,'-e','/usr/bin/python3',__file__,'probe',case],stderr=log,stdout=subprocess.DEVNULL,env={**os.environ,'TERMINAL_RENDERER_DIAGNOSTICS':'1','TERMINAL_CONFIG':str(cfg),'NATIVE_BOLD_FAMILY':('Noto Mono' if label=='fallback' else 'JetBrainsMono Nerd Font Mono')})
            time.sleep(3)
            assert proc.poll() is None,(case,'app exited early')
            subprocess.run(['spectacle','-a','-b','-n','-o',str(ROOT/f'{label}-{case}.png')],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
            assert proc.wait(timeout=20)==0,case
            print(label,case,'captured and exited',flush=True)
