"""1.5 saved-example invariance, independent trace, structure/glass/checkpoint verification."""
import copy, hashlib, json, time
from pathlib import Path
import validate_rayoptics as ref
ROOT=Path(__file__).resolve().parents[1]
ref.CLI=ROOT/'target/release/oos-cli.exe'
old=json.loads((ROOT/'docs/validation-design-1.4.json').read_text(encoding='utf8'))
records=[]
for previous in old['unchanged_examples']:
    name=previous['example']; path=ROOT/f'examples/{name}.oos.json'; raw=path.read_bytes();p=json.loads(raw)
    a=ref.invoke(dict(operation='analyze',project=p,sampling=33))
    rms=[f['polychromatic_rms_um'] for f in a['fields']]
    assert rms==previous['fields_rms_um'],(name,rms,previous['fields_rms_um'])
    roundtrip=json.loads(json.dumps(p));roundtrip.setdefault('workspace',{})['search_checkpoint']={"test_metadata_only":True}
    assert a==ref.invoke(dict(operation='analyze',project=roundtrip,sampling=33))
    resolved=copy.deepcopy(p);resolved['fields']=[f['field'] for f in a['fields']];resolved.pop('field_spec',None)
    cross=ref.validate(resolved)
    assert hashlib.sha256(raw).hexdigest()==previous['source_sha256']
    records.append(dict(example=name,unchanged_rms=True,source_sha256=previous['source_sha256'],rayoptics=cross))
    print(name,'unchanged',flush=True)
b=dict(structure='tessar',focal_length_mm=50,f_number=8,sensor_mm=[6,4],half_field_deg=5,max_length_mm=150,max_rms_um=25,iterations=10,starts=1,require_sensor_coverage=True,glass_search=False,exploration='expanded',element_glass=True,max_attempts=6,min_center_mm=.5,min_edge_mm=.1)
cp=None;steps=[];t=time.perf_counter()
for i in range(6):
    r=ref.invoke(dict(operation='synthesize_step',project=p,design_brief=b,search_checkpoint=cp))
    cp=json.loads(json.dumps(r['checkpoint']))
    assert cp['cursor']==i+1 and cp['total']==6
    steps.append(cp['attempts'][-1]);print('search',i+1,steps[-1],flush=True)
assert r['done'] and r['outcome']['candidate'] and r['outcome']['candidate']['accepted']
assert any('后置校正片' in x for x in steps) and any('第 1 片独立换玻璃' in x for x in steps)
best=r['outcome']['candidate'];a=ref.invoke(dict(operation='analyze',project=best['project'],sampling=49))
assert all(s['failed']==0 for s in a['spots'])
assert all(f['polychromatic_rms_um']<=b['max_rms_um'] for f in a['fields'])
resolved=copy.deepcopy(best['project']);resolved['fields']=[f['field'] for f in a['fields']];resolved.pop('field_spec',None)
cross=ref.validate(resolved)
checked=ref.invoke(dict(operation='synthesize_step',project=p,design_brief=b,search_checkpoint=cp))
assert checked['done'] and checked['checkpoint']['cursor']==6
assert checked['outcome']['candidate']['accepted']
assert checked['outcome']['candidate']['project']==best['project']
impossible=ref.invoke(dict(operation='synthesize',project=p,design_brief=dict(b,max_length_mm=1,iterations=1,max_attempts=1)))
assert not impossible['candidate'] or not impossible['candidate']['accepted']
out=dict(version='1.5.0',unchanged_examples=records,search=dict(brief=b,elapsed_seconds=time.perf_counter()-t,attempts=steps,candidates=[dict(name=c['project']['name'],accepted=c['accepted'],rms_um=c['after_rms_um'],surface_count=len(c['project']['surfaces']),checks=c['checks']) for c in r['outcome']['candidates']],best_rayoptics=cross,verification_sampling=49,best_fields_rms_um=[f['polychromatic_rms_um'] for f in a['fields']]),checkpoint_roundtrip_and_revalidation=True,impossible_length_rejected=True)
(ROOT/'docs/validation-design-1.5.json').write_text(json.dumps(out,ensure_ascii=False,indent=2)+'\n',encoding='utf8')
print('design verification passed',flush=True)
