import pathlib, subprocess, json
root=pathlib.Path('/tmp/bold-intensity-evidence/verified')
def image(name):
    p=root/name
    w,h=map(int,subprocess.check_output(['magick','identify','-format','%w %h',str(p)],text=True).split())
    return w,h,subprocess.check_output(['magick',str(p),'-depth','8','rgb:-'])
results=[]
for scale in ['1','1.25','2']:
    base=image(f'{scale}-static-16-dark-baseline.png')
    for intensity in ['None','1.0']:
        candidate=image(f'{scale}-static-16-dark-{intensity}.png')
        assert candidate==base,(scale,intensity)
        results.append(f'{scale}x: original HEAD vs {intensity}: entire screenshot identical')
    for family in ['static','variable']:
        normal=image(f'{scale}-{family}-16-dark-1.0.png')
        w,h,data=normal
        factor=float(scale)
        # Text rows begin at the physical scaled title-bar height (30 logical px).
        # Grid metrics come from the actual diagnostic log, rather than scaling estimates.
        import re
        log=(root/f'{scale}-{family}-16-dark-1.0.log').read_text()
        cell_height=int(re.search(r'cell_metrics=CellMetrics \{ width: \d+, height: (\d+)',log).group(1))
        sample=re.search(r'event=glyph-geometry row=2 cell_y=(\d+)',log)
        assert sample, (scale,family)
        top=int(sample.group(1))-2*cell_height
        for intensity in ['0.75','1.25']:
            cw,ch,other=image(f'{scale}-{family}-16-dark-{intensity}.png')
            assert (cw,ch)==(w,h)
            for row in [0,1,2,4,5,6]:
                start=(top+row*cell_height)*w*3
                end=(top+(row+1)*cell_height)*w*3
                assert data[start:end]==other[start:end],(scale,family,intensity,row)
            start=(top+3*cell_height)*w*3
            end=(top+4*cell_height)*w*3
            assert data[start:end]!=other[start:end],(scale,family,intensity,'bold effect')
            results.append(f'{scale}x {family} {intensity}: regular/reset/regular color rows identical; Bold row differs')
fallback=image('2-fallback-16-dark-1.0.png')
for intensity in ['0.75','1.25']:
    assert image(f'2-fallback-16-dark-{intensity}.png')==fallback
    results.append(f'2x Noto Mono {intensity}: entire screenshot unchanged')
for scale,size in [('1',12),('2',20)]:
    for family in ['static','variable']:
        w,h,data=image(f'{scale}-{family}-{size}-light-1.0.png')
        log=(root/f'{scale}-{family}-{size}-light-1.0.log').read_text()
        cell_height=int(re.search(r'cell_metrics=CellMetrics \{ width: \d+, height: (\d+)',log).group(1))
        top=int(re.search(r'event=glyph-geometry row=2 cell_y=(\d+)',log).group(1))-2*cell_height
        for intensity in ['0.75','1.25']:
            cw,ch,other=image(f'{scale}-{family}-{size}-light-{intensity}.png')
            assert (cw,ch)==(w,h)
            for row in [0,1,2,4,5,6,9,10]:
                start=(top+row*cell_height)*w*3;end=(top+(row+1)*cell_height)*w*3
                assert data[start:end]==other[start:end],(scale,family,size,intensity,row)
            start=(top+3*cell_height)*w*3;end=(top+4*cell_height)*w*3
            assert data[start:end]!=other[start:end]
            results.append(f'{scale}x {family} {size}px light {intensity}: regular/reset/color/blank rows identical; Bold row differs')
(root/'pixel-comparison.txt').write_text('\n'.join(results)+'\n')
print('\n'.join(results))
