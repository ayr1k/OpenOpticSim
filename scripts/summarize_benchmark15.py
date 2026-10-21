import json, statistics
from pathlib import Path
import validate_rayoptics as ref
ROOT=Path(__file__).resolve().parents[1];ref.CLI=ROOT/'target/release/oos-cli.exe'
baseline=json.loads((ROOT/'docs/benchmark-design-1.4.json').read_text(encoding='utf8'))
current=json.loads((ROOT/'docs/benchmark-design-1.5.json').read_text(encoding='utf8'))
rows=[]
for old,new in zip(baseline['cases'],current['cases']):
    assert old['brief']==new['brief']
    c=new['result'].get('candidate');verification=None
    if new['accepted']:
        a=ref.invoke(dict(operation='analyze',project=c['project'],sampling=49))
        assert all(s['failed']==0 for s in a['spots'])
        assert all(f['polychromatic_rms_um']<=new['brief']['max_rms_um'] for f in a['fields'])
        assert all(v['status']=='sampled_clear' for v in a['coverage']['checkpoints'])
        p=dict(c['project'],fields=[f['field'] for f in a['fields']]);p.pop('field_spec',None)
        verification=dict(sampling=49,fields_rms_um=[f['polychromatic_rms_um'] for f in a['fields']],rayoptics=ref.validate(p))
    rows.append(dict(brief=new['brief'],v14_passed=old['accepted'],v15_passed=new['accepted'],v14_seconds=old['elapsed_seconds'],v15_seconds=new['elapsed_seconds'],v15_verification=verification))
out=dict(version='1.5.0',cases=rows,v14_successes=sum(r['v14_passed'] for r in rows),v15_successes=sum(r['v15_passed'] for r in rows),total=len(rows),v14_median_seconds=statistics.median(r['v14_seconds'] for r in rows),v15_median_seconds=statistics.median(r['v15_seconds'] for r in rows),
         scope='Four classic topologies; 50mm; F8 6x4, F4 6x4, F8 36x24; one start, fixed glass, original topology, 25 iterations/stage. This is a small fixed benchmark, not a general success probability.',
         differences='Same user-visible iteration budget, not equal evaluations or wall time. 1.5 adds restoration and up to two retry stages, uses explicit 0.5mm center/0.1mm body minimum; 1.4 scaled minima with seed. Times on shared development hardware are indicative, not a speed comparison.',
         executable_hashes={v:d['executable_sha256'] for v,d in [('1.4',baseline),('1.5',current)]})
(ROOT/'docs/validation-benchmark-1.5.json').write_text(json.dumps(out,ensure_ascii=False,indent=2)+'\n',encoding='utf8')
print(json.dumps({k:v for k,v in out.items() if k not in ['cases']},ensure_ascii=False))
