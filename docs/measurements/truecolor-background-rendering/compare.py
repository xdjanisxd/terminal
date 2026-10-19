import os, sys, time, pathlib, subprocess, pty, fcntl, termios, struct, select, tty, json
REPO=pathlib.Path(__file__).resolve().parents[3]
ROOT=pathlib.Path(os.environ.get('TRUECOLOR_EVIDENCE','/tmp/truecolor-background-rendering'))
SELF=str(pathlib.Path(__file__).resolve())

def fixture():
    out=b'\x1b[?25l\x1b[0m\x1b[2J\x1b[H'
    def at(row,text): return f'\x1b[{row};1H'.encode()+text
    out+=at(1,b'Painted spaces / erase to EOL, identical active RGB')
    for row,(r,g,b) in enumerate([(32,32,32),(57,57,57),(68,68,68),(120,169,255)],3):
        out+=at(row,f'\x1b[48;2;{r};{g};{b}m'.encode()+b' PAINT '+b' '*18+b' ERASE '+b'\x1b[K\x1b[0m')
    out+=at(8,b'\x1b[38;2;120;169;255mRGB cyan regular \x1b[1mBold \x1b[22mreset\x1b[0m')
    out+=at(9,b'\x1b[31mANSI red \x1b[91mbright \x1b[38;5;196m256 red\x1b[0m')
    out+=at(11,b'\x1b[48;2;32;32;32m'+b' '*60+b'\x1b[1G\x1b[30CX\x1b[5X\x1b[0m')
    out+=at(13,b'\x1b[48;2;57;57;57m\x1b[2K\x1b[0mEL2')
    out+=at(15,b'\x1b[38;2;120;169;255;48;2;32;32;32;7mInverse\x1b[27m normal\x1b[49m default background\x1b[0m')
    out+=at(17,b'\x1b[48;2;32;32;32mwide: '+ '日本語 e\u0301'.encode()+b'\x1b[0m')
    return out

def ansi_fixture():
    out=b'\x1b[?25l\x1b[0m\x1b[2J\x1b[HANSI16 and 256 backgrounds'
    for i in range(16):
        code=(40+i) if i<8 else (100+i-8)
        out+=f'\x1b[2;{i*4+1}H\x1b[{code}m    '.encode()
    for i in range(256):
        row,col=divmod(i,16)
        out+=f'\x1b[{row+4};{col*4+1}H\x1b[48;5;{i}m    '.encode()
    return out+b'\x1b[0m'

if len(sys.argv)>1 and sys.argv[1]=='probe':
    _,_,case,label=sys.argv
    tty.setraw(0)
    time.sleep(float(os.environ.get('TRUECOLOR_DELAY','1.5' if os.environ.get('TRUECOLOR_POSITION') else '0')))
    (ROOT/(label+'.env.json')).write_text(json.dumps({k:os.environ.get(k) for k in ['TERM','COLORTERM','WAYLAND_DISPLAY','TERMINAL_CONFIG']},indent=2))
    if case in ['fixture','replay','ansi']:
        data=fixture() if case=='fixture' else ansi_fixture() if case=='ansi' else (ROOT/'source-frame.ansi').read_bytes();(ROOT/(label+'.ansi')).write_bytes(data);os.write(1,data);time.sleep(6);sys.exit()
    pid,fd=pty.fork()
    if pid==0:
        fcntl.ioctl(0,termios.TIOCSWINSZ,struct.pack('HHHH',24,80,0,0))
        os.environ['TERM']=os.environ.get('TRUECOLOR_INNER_TERM',os.environ.get('TERM','xterm-256color'))
        args=['/usr/bin/vim' if case=='vim' else '/usr/bin/nvim','-n']
        if case=='zsh': os.execv('/usr/bin/zsh',['zsh','-f'])
        if case in ['nvim-min','vim']: args+=['-u',str(ROOT/'minimal.vim')]
        if case=='vim': args+=['-i','NONE','--cmd','set shortmess+=F']
        if case!='zsh':
            dump="lua local t={}; for _,n in ipairs({'Normal','CursorLine','Visual','Search','IncSearch','LineNr','StatusLine'}) do t[n]=vim.api.nvim_get_hl(0,{name=n,link=false}) end; vim.fn.writefile({vim.json.encode({highlights=t,termguicolors=vim.o.termguicolors,background=vim.o.background})},'"+str(ROOT/(label+'.highlights.json'))+"')"
            if case!='vim':args+=['-c',dump]
        args+=['-c',"call timer_start(5500, {-> execute('qa!')})",'-c',"call timer_start(1500, {-> execute('redraw!')})",str(ROOT/'sample.rs')]
        os.execv(args[0],args)
    fcntl.ioctl(fd,termios.TIOCSWINSZ,struct.pack('HHHH',24,80,0,0))
    transcript=bytearray(); replies=bytearray(); start=time.monotonic()
    frame_saved=False;sent=False;quitting=False
    while time.monotonic()-start<9:
        elapsed=time.monotonic()-start
        if elapsed>2.5 and not frame_saved:
            (ROOT/(label+'.frame.ansi')).write_bytes(transcript);frame_saved=True
        if case=='zsh' and elapsed>.5 and not sent:
            os.write(fd,b"source /usr/share/zsh-syntax-highlighting/zsh-syntax-highlighting.zsh; ZSH_HIGHLIGHT_STYLES[command]='fg=green,bold'; ZSH_HIGHLIGHT_STYLES[unknown-token]='fg=red,bold'; PROMPT='Colors %> '\n")
            time.sleep(.1);os.write(fd,b'echo \"syntax highlighting\" && invalid-command');sent=True
        if case=='zsh' and elapsed>5.5 and not quitting:
            os.write(fd,b'\x03exit\n');quitting=True
        ready=select.select([fd,0],[],[],.05)[0]
        if 0 in ready:
            data=os.read(0,65536);replies+=data;os.write(fd,data)
        if fd in ready:
            try: data=os.read(fd,65536)
            except OSError: break
            if not data: break
            transcript+=data;os.write(1,data)
    (ROOT/(label+'.ansi')).write_bytes(transcript)
    (ROOT/(label+'.replies')).write_bytes(replies)
    os.close(fd);os.waitpid(pid,0);sys.exit()

ROOT.mkdir(exist_ok=True)
(ROOT/'sample.rs').write_text('// color background reproduction\nfn main() {\n    let color = "RGB and default";\n    println!("{color}");\n}\n\n'+('\n'.join(f'// region {i}: row background and syntax' for i in range(30))))
(ROOT/'minimal.vim').write_text('set termguicolors background=dark number cursorline noswapfile\nsyntax on\nhighlight Normal guifg=#d0d0d0 guibg=#161616\nhighlight CursorLine guibg=#202020\nhighlight Visual guibg=#393939\nhighlight Search guifg=#ffffff guibg=#444444\nhighlight IncSearch guifg=#161616 guibg=#78a9ff\n')
label=sys.argv[1];which=sys.argv[2];cases=sys.argv[3:] or ['fixture','nvim-min','nvim-config']
palette=['#10141c','#ff1744','#00e65c','#ffd600','#1673ff','#b900ff','#00d9e8','#d2d8e2','#586174','#ff3158','#00f577','#ffe533','#3688ff','#c83aff','#19e7f2','#f2f5fa']
opacity=float(os.environ.get('TRUECOLOR_OPACITY','1.0'));blur=os.environ.get('TRUECOLOR_BLUR','false')
(ROOT/'terminal.toml').write_text(f'[font]\nfamily="JetBrainsMono Nerd Font Mono"\nsize=16\nbold_intensity=1.0\n[window]\nopacity={opacity}\nblur={blur}\n[theme]\nforeground="#d0d0d0"\nbackground="#05070b"\nansi = '+json.dumps(palette)+'\n')
(ROOT/'alacritty.toml').write_text(f'[window]\nopacity={opacity}\nblur={blur}\ndimensions={{columns=100,lines=36}}\n[font]\nnormal={{family="JetBrainsMono Nerd Font Mono",style="Regular"}}\nbold={{family="JetBrainsMono Nerd Font Mono",style="Bold"}}\nsize=12\n[colors]\ntransparent_background_colors=true\ndraw_bold_text_with_bright_colors=false\n[colors.primary]\nforeground="#d0d0d0"\nbackground="#05070b"\n')
with (ROOT/'alacritty.toml').open('a') as config:
    for section,values in [('normal',palette[:8]),('bright',palette[8:])]:
        config.write('\n[colors.'+section+']\n')
        for key,value in zip(['black','red','green','yellow','blue','magenta','cyan','white'],values):
            config.write(key+'=\"'+value+'\"\n')
for case in cases:
    name=label+'-'+case
    binary=str(ROOT/'terminal-before') if which=='before' else str(REPO/'target/release/terminal')
    args=([binary] if which!='alacritty' else ['/usr/bin/alacritty','--config-file',str(ROOT/'alacritty.toml')])+['-e','/usr/bin/python3',SELF,'probe',case,name]
    env={**os.environ,'TERMINAL_CONFIG':str(ROOT/'terminal.toml'),'TERMINAL_RENDERER_DIAGNOSTICS':'1','WINIT_UNIX_BACKEND':'wayland','NVIM_LOG_FILE':str(ROOT/(name+'.nvim.log'))}
    with (ROOT/(name+'.log')).open('w') as log:
        app=subprocess.Popen(args,env=env,stdout=log,stderr=log,cwd=ROOT)
        try:
            time.sleep(1.0)
            if os.environ.get('TRUECOLOR_POSITION'):
                x,y=map(int,os.environ['TRUECOLOR_POSITION'].split(','))
                script=ROOT/'position.js';plugin='truecolor-position-'+str(app.pid)
                script.write_text(f"for (const w of workspace.windowList()) {{ if (w.pid === {app.pid}) {{ w.setMaximize(false,false); w.frameGeometry = {{x:{x},y:{y},width:840,height:600}}; w.keepAbove=true; workspace.activeWindow=w; }} }}")
                ident=subprocess.check_output(['qdbus6','org.kde.KWin','/Scripting','org.kde.kwin.Scripting.loadScript',str(script),plugin],text=True).strip()
                subprocess.run(['qdbus6','org.kde.KWin','/Scripting/Script'+ident,'org.kde.kwin.Script.run'],check=True,stdout=subprocess.DEVNULL)
                subprocess.run(['qdbus6','org.kde.KWin','/Scripting','org.kde.kwin.Scripting.unloadScript',plugin],check=True,stdout=subprocess.DEVNULL)
            time.sleep(2.5)
            assert app.poll() is None,(name,app.returncode)
            subprocess.run(['spectacle','-a','-b','-n','-o',str(ROOT/(name+'.png'))],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
            if os.environ.get('TRUECOLOR_DESKTOP'):
                subprocess.run(['spectacle','-f','-b','-n','-o',str(ROOT/(name+'-desktop.png'))],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
            status=app.wait(timeout=12)
            print(name,'exit',status,flush=True)
        finally:
            if app.poll() is None: app.terminate();app.wait(timeout=5)
