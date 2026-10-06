import csv, json, re, statistics as st
from pathlib import Path

root = Path('target/font-discovery')
rows = []
for d in sorted(root.iterdir()):
    if not d.is_dir() or d.name in {'baseline-a', 'final-registry-smoke'} or not (d / 'samples.csv').exists():
        continue
    costs = {}
    for row in csv.DictReader((d / 'costs.csv').open()):
        costs.setdefault(int(row['run']), {})[row['stage']] = int(row['duration_us'])
    for row in csv.DictReader((d / 'samples.csv').open()):
        run = int(row['run'])
        result = dict(row, batch=d.name, run=run)
        result.update(costs[run])
        result['wgpu-core-init'] = sum(costs[run][s] for s in ['wgpu-instance','adapter-request','device-request'])
        result['font-preparation-plus-metrics'] = result['font-family-preparation'] + result['font-metrics']
        if (d / f'run-{run}.fontdb').exists():
            for stage, us, count in re.findall(r'stage=(\S+) duration_us=(\d+) count=(\d+)', (d / f'run-{run}.fontdb').read_text()):
                result['dep-' + stage] = int(us)
                result['count-' + stage] = int(count)
            result['dep-directory-other'] = result['dep-discovery'] - result['dep-file-total'] - result['dep-entry-type-canonicalize'] - result['dep-read-dir-open']
            result['dep-file-other'] = result['dep-file-total'] - result['dep-file-open-map'] - result['dep-face-metadata-parse'] - result['dep-file-unmap-close']
        rows.append(result)

dest = Path('docs/measurements/font-discovery-startup')
dest.mkdir(parents=True, exist_ok=True)
keys = sorted(set().union(*(r.keys() for r in rows)))
with (dest / 'runs.csv').open('w', newline='') as f:
    writer = csv.DictWriter(f, keys)
    writer.writeheader()
    writer.writerows(rows)

stages = ['font-family-preparation','font-metrics','fonts_to_frame_us','system-font-discovery','fallback-candidate-sort','font-database-create','font-generic-family-query','font-configured-family-query','font-primary-monospace-validation','fallback-candidate-collection','font-cache-setup','dep-discovery','dep-file-open','dep-file-map','dep-file-unmap-close','dep-face-metadata-parse','dep-file-other','dep-entry-type-canonicalize','dep-read-dir-open','dep-directory-other','wgpu-core-init','font-preparation-plus-metrics']
summary = {}
for batch in sorted(set(r['batch'] for r in rows)):
    batch_rows = [r for r in rows if r['batch'] == batch]
    summary[batch] = {'n': len(batch_rows)}
    for stage in stages:
        values = [int(r[stage])/1000 for r in batch_rows if stage in r]
        if values:
            summary[batch][stage] = dict(median=round(st.median(values),3), min=round(min(values),3), max=round(max(values),3))
paired = {}
for stage in stages:
    deltas = []
    for i in range(1,11):
        a = next((r for r in rows if r['batch'] == f'pair-{i}-baseline'), None)
        b = next((r for r in rows if r['batch'] == f'pair-{i}-candidate'), None)
        if a and b and stage in a and stage in b:
            deltas.append((int(b[stage])-int(a[stage]))/1000)
    if deltas:
        paired[stage] = dict(deltas=[round(x,3) for x in deltas], median=round(st.median(deltas),3), min=round(min(deltas),3),max=round(max(deltas),3))
summary['paired-deltas-candidate-minus-baseline'] = paired
overhead = {}
for stage in ['font-family-preparation','system-font-discovery','fonts_to_frame_us']:
    deltas = []
    for i in range(1,11):
        a = next((r for r in rows if r['batch'] == f'overhead-{i}-off'), None)
        b = next((r for r in rows if r['batch'] == f'overhead-{i}-on'), None)
        if a and b:
            deltas.append((int(b[stage])-int(a[stage]))/1000)
    overhead[stage] = dict(deltas=[round(x,3) for x in deltas], median=round(st.median(deltas),3), min=round(min(deltas),3), max=round(max(deltas),3))
summary['overhead-on-minus-off'] = overhead
coarse = {}
for stage in stages[:5]:
    deltas = []
    for i in range(1,11):
        a = next((r for r in rows if r['batch'] == f'coarse-{i}-baseline'), None)
        b = next((r for r in rows if r['batch'] == f'coarse-{i}-candidate'), None)
        if a and b:
            deltas.append((int(b[stage])-int(a[stage]))/1000)
    if deltas:
        coarse[stage] = dict(deltas=[round(x,3) for x in deltas], median=round(st.median(deltas),3), min=round(min(deltas),3), max=round(max(deltas),3))
summary['coarse-deltas-candidate-minus-baseline'] = coarse
for variant in ['baseline','candidate']:
    subset = [r for r in rows if re.fullmatch('coarse-\\d+-'+variant, r['batch'])]
    if subset:
        summary['coarse-'+variant] = {s: dict(median=round(st.median([int(r[s])/1000 for r in subset if s in r]),3), min=round(min(int(r[s])/1000 for r in subset if s in r),3), max=round(max(int(r[s])/1000 for r in subset if s in r),3)) for s in stages if any(s in r for r in subset)}
for variant in ['baseline','candidate']:
    subset = [r for r in rows if re.fullmatch('pair-\\d+-'+variant, r['batch'])]
    summary['paired-'+variant] = {s: dict(median=round(st.median([int(r[s])/1000 for r in subset if s in r]),3), min=round(min(int(r[s])/1000 for r in subset if s in r),3), max=round(max(int(r[s])/1000 for r in subset if s in r),3)) for s in stages if any(s in r for r in subset)}
(dest / 'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
for batch in ['baseline-b','candidate-b','baseline-consolas','candidate-consolas']:
    if batch in summary:
        print(batch,{s: summary[batch].get(s) for s in stages[:5]})
print('paired',{s:paired[s] for s in stages[:5]})
print('overhead',overhead)
print('paired medians',{v:{s:summary['paired-'+v][s] for s in stages[:5]} for v in ['baseline','candidate']})
print('breakdown',{s:summary['baseline-b'].get(s) for s in stages[5:]})
print('coarse',coarse)
print('coarse medians',{v:{s:summary.get('coarse-'+v,{}).get(s) for s in stages[:5]} for v in ['baseline','candidate']})
