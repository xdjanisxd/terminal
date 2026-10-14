import subprocess, pathlib, json
root=pathlib.Path('/tmp/bold-intensity-evidence/final')
root.mkdir(parents=True,exist_ok=True)
commands=[
 ['cargo','fmt','--all','--','--check'],
 ['cargo','check','--workspace','--all-targets','--offline'],
 ['cargo','test','--workspace','--all-targets','--offline'],
 ['cargo','test','--workspace','--doc','--offline'],
 ['cargo','clippy','--workspace','--all-targets','--offline','--','-D','warnings'],
 ['cargo','build','--release','-p','terminal-app','--offline'],
 ['git','diff','--check'],
]
results=[]
for index,cmd in enumerate(commands):
 with (root/f'rust-gate-{index}.log').open('w') as log:
  result=subprocess.run(cmd,stdout=log,stderr=subprocess.STDOUT)
 results.append({'command':cmd,'exit_code':result.returncode})
 (root/'rust-gates.json').write_text(json.dumps(results,indent=2)+'\n')
 print(' '.join(cmd),result.returncode,flush=True)
 if result.returncode: raise SystemExit(result.returncode)
(root/'terminal-candidate').write_bytes(pathlib.Path('target/release/terminal').read_bytes())
(root/'terminal-candidate').chmod(0o755)
(root/'terminal-baseline').write_bytes(pathlib.Path('/tmp/bold-intensity-evidence/terminal-baseline').read_bytes())
(root/'terminal-baseline').chmod(0o755)
