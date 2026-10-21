"""Same briefs/iteration budget on frozen 1.4 and current executable; retain raw evidence."""
import hashlib, json, platform, subprocess, sys, time
from pathlib import Path
ROOT = Path(__file__).resolve().parents[1]
version = sys.argv[1]
cli = ROOT / ('artifacts/baseline-1.4/oos-cli.exe' if version == '1.4' else 'artifacts/benchmark-1.5.exe')
p = json.loads((ROOT/'examples/tessar-50mm-1.3.oos.json').read_text(encoding='utf8'))
rows = []
for structure in ['tessar', 'planar', 'double_gauss', 'sonnar']:
    for fnumber, sensor in [(8, [6,4]), (4, [6,4]), (8, [36,24])]:
        brief = dict(structure=structure, focal_length_mm=50, f_number=fnumber,
                     sensor_mm=sensor, half_field_deg=5, max_length_mm=150,
                     max_rms_um=25, iterations=25, starts=1,
                     require_sensor_coverage=True, glass_search=False)
        t = time.perf_counter()
        r = subprocess.run([str(cli), '--request'], input=json.dumps(dict(operation='synthesize',project=p,design_brief=brief)),
                           text=True, encoding='utf8', capture_output=True)
        result = json.loads(r.stdout) if r.returncode == 0 else dict(error=r.stderr)
        c = result.get('candidate')
        rows.append(dict(brief=brief, elapsed_seconds=time.perf_counter()-t,
                         accepted=bool(c and c['accepted']), result=result))
        print(structure, fnumber, sensor, rows[-1]['accepted'], round(rows[-1]['elapsed_seconds'],2), flush=True)
        out = dict(version=version, executable_sha256=hashlib.sha256(cli.read_bytes()).hexdigest(),
                   platform=platform.platform(), processor=platform.processor(),
                   budget='25 LM iterations per continuation stage, one start, original topology, fixed glass pair. Same user iteration budget, NOT equal evaluation/wall-time budget: restoration and up to two intermediate-stage retries add work in 1.5. Timings are indicative on a shared development machine.', cases=rows)
        (ROOT/f'docs/benchmark-design-{version}.json').write_text(json.dumps(out,ensure_ascii=False,indent=2)+'\n',encoding='utf8')
