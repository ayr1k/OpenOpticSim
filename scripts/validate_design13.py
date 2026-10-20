"""Repeat classic topology synthesis and independently verify resulting intersections."""
import json,time
from pathlib import Path
import validate_rayoptics as ref
ROOT=Path(__file__).resolve().parents[1]
ref.CLI=ROOT/'target/release/oos-cli.exe'
source=json.loads((ROOT/'examples/400mm-repaired.oos.json').read_text(encoding='utf8'))
records=[]
for kind in ['tessar','planar','double_gauss','sonnar']:
    brief=dict(structure=kind,focal_length_mm=50.,f_number=8.,sensor_mm=[36.,24.],
               half_field_deg=5.,max_length_mm=150.,max_rms_um=25.,iterations=25,starts=1)
    t=time.perf_counter()
    outcome=ref.invoke(dict(operation='synthesize',project=source,design_brief=brief))
    candidate=outcome['candidate']
    assert candidate and candidate['accepted'],outcome['attempts']
    assert all(c['passed'] for c in candidate['checks'])
    p=candidate['project']
    check=ref.validate(p)
    p['name']=f'{kind} · 50 mm F8 · 5deg teaching design'
    p['description']+=' Validated at 5 degree half-field only; full-frame corners and manufacturing not certified.'
    target=ROOT/f'examples/{kind}-50mm-1.3.oos.json'
    target.write_text(json.dumps(p,ensure_ascii=False,indent=2)+'\n',encoding='utf8')
    reopened=json.loads(target.read_text(encoding='utf8'))
    a=ref.invoke(dict(operation='analyze',project=p,sampling=33))
    b=ref.invoke(dict(operation='analyze',project=reopened,sampling=33))
    assert a==b
    records.append(dict(structure=kind,brief=brief,elapsed_seconds=time.perf_counter()-t,
                        rms_um=candidate['after_rms_um'],checks=candidate['checks'],
                        rayoptics=check,roundtrip_exact=True,
                        efl_mm=a['paraxial']['efl'],f_number=a['paraxial']['f_number']))
impossible=dict(brief,max_length_mm=1.)
r=ref.invoke(dict(operation='synthesize',project=source,design_brief=impossible))
assert r['candidate'] is None or not r['candidate']['accepted']
report=dict(version='1.3.0',classic_structures=records,impossible_length_rejected=True,
            scope='Nominal geometric 50mm F8 +/-5deg; no full-frame, MTF or manufacturing certification')
(ROOT/'docs/validation-design-1.3.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n',encoding='utf8')
print(json.dumps({r['structure']:r['rms_um'] for r in records}))
