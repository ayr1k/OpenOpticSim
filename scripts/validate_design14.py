"""v1.4 bounded search, coverage and unchanged saved prescriptions.

Does not rewrite examples or previous release evidence.
"""
import copy
import hashlib
import json
import time
from pathlib import Path
import validate_rayoptics as reference

ROOT = Path(__file__).resolve().parents[1]
reference.CLI = ROOT / 'target/release/oos-cli.exe'
invoke = reference.invoke
records = []
for name in ['24-70mm-zoom', '400mm-repaired', 'tessar-50mm-1.3',
             'planar-50mm-1.3', 'double_gauss-50mm-1.3', 'sonnar-50mm-1.3']:
    path = ROOT / f'examples/{name}.oos.json'
    original = path.read_bytes()
    p = json.loads(original)
    a = invoke(dict(operation='analyze', project=p, sampling=33))
    reopened = json.loads(json.dumps(p))
    reopened.setdefault('workspace', dict(schema_version=1, versions=[]))['design_task'] = dict(
        structure='auto', focal_length_mm=50, f_number=8, sensor_mm=[6, 4],
        half_field_deg=5, max_length_mm=150, max_rms_um=25, starts=1, iterations=25,
        require_sensor_coverage=True, glass_search=True)
    assert a == invoke(dict(operation='analyze', project=reopened, sampling=33))
    physical = copy.deepcopy(p)
    physical['fields'] = [f['field'] for f in a['fields']]
    physical.pop('field_spec', None)
    cross = reference.validate(physical)
    assert original == path.read_bytes()
    records.append(dict(example=name, source_sha256=hashlib.sha256(original).hexdigest(),
                        saved_workspace_invariance=True, rayoptics=cross,
                        fields_rms_um=[f['polychromatic_rms_um'] for f in a['fields']],
                        coverage=a['coverage']))

brief = reopened['workspace']['design_task']
t = time.perf_counter()
result = invoke(dict(operation='synthesize', project=p, design_brief=brief))
assert len(result['attempts']) == 12
assert result['candidate'] and result['candidate']['accepted'], result['attempts']
candidates = result['candidates']
assert candidates == sorted(candidates, key=lambda c: (not c['accepted'], c['after_rms_um']))
for candidate in candidates:
    assert candidate['accepted'] == all(c['passed'] for c in candidate['checks'])
    analysis = invoke(dict(operation='analyze', project=candidate['project'], sampling=33))
    if candidate['accepted']:
        assert all(c['status'] == 'sampled_clear' for c in analysis['coverage']['checkpoints'])
        assert all(f['polychromatic_rms_um'] <= brief['max_rms_um'] + 1e-9 for f in analysis['fields'])
best = result['candidate']['project']
best_a = invoke(dict(operation='analyze', project=best, sampling=33))
resolved = copy.deepcopy(best)
resolved['fields'] = [f['field'] for f in best_a['fields']]
resolved.pop('field_spec', None)
cross = reference.validate(resolved)
impossible = invoke(dict(operation='synthesize', project=p,
                        design_brief=dict(brief, structure='tessar', glass_search=False,
                                          sensor_mm=[100, 100], focal_length_mm=10, iterations=1)))
assert not impossible['candidate'] or not impossible['candidate']['accepted']
out = dict(version='1.4.0', unchanged_examples=records,
           search=dict(brief=brief, elapsed_seconds=time.perf_counter()-t,
                       attempts=result['attempts'],
                       candidates=[dict(name=c['project']['name'], accepted=c['accepted'],
                                        rms_um=c['after_rms_um'], checks=c['checks']) for c in candidates],
                       best_rayoptics=cross, best_coverage=best_a['coverage']),
           unreachable_sensor_rejected=True,
           limitations='6x4 mm sensor at 50 mm F8 benchmark only; no full-frame or manufacturing claim')
(ROOT/'docs/validation-design-1.4.json').write_text(json.dumps(out, ensure_ascii=False, indent=2)+'\n', encoding='utf8')
print(json.dumps(dict(attempts=len(result['attempts']), candidates=len(candidates),
                      accepted=sum(c['accepted'] for c in candidates),
                      best=result['candidate']['after_rms_um'], unreachable_rejected=True)))
