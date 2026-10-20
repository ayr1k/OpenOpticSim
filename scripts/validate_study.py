"""v1.2 fixed-sensor studies, independent intersections, and legacy motion regression."""
import copy
import json
import sys
from pathlib import Path
import validate_rayoptics as reference
from validate_motion import options as motion_options

ROOT=Path(__file__).resolve().parents[1]
reference.CLI=ROOT/'target/release/oos-cli.exe'
invoke=reference.invoke

def settings(m):
    return dict(mode='scan',motion=motion_options(m,m['configurations'][0],7),configurations=m['configurations'],
                steps=5,max_points=41,adaptive=True,refine_rms_um=3.,driver_group=0,compensator_group=1,
                near_mm=10000.,far_mm=100000.,include_infinity=True,verification_sampling=17,
                max_degradation_percent=0.,efl_tolerance_percent=None,min_glass_mm=0.,require_sensor_coverage=True,
                variables=[],iterations=8)

def run(p,o):
    return invoke(dict(operation='motion_study',project=p,study_options=o))

def independent(p,o,point):
    c=point['configuration']; op=copy.deepcopy(o['motion'])
    op.update(positions_mm=c['positions_mm'],object_distance=c['object_distance'],target_f_number=c['target_f_number'],solve_group=None,sampling=17)
    r=invoke(dict(operation='motion',project=p,motion_options=op))
    assert abs(r['score_um']-point['rms_um'])<1e-10
    assert abs(sum(s['thickness'] for s in r['project']['surfaces'])+r['origin_shift_mm']-sum(s['thickness'] for s in p['surfaces']))<1e-8
    q=r['project'];q['fields']=[f['field'] for f in r['analysis']['fields']];q.pop('field_spec',None)
    return reference.validate(q)

def main():
    zoom=json.loads((ROOT/'examples/24-70mm-zoom.oos.json').read_text(encoding='utf8'))
    m=zoom['workspace']['motion'];p=m['reference'];o=settings(m)
    scan=run(p,o)
    assert scan['accepted'] and len(scan['points'])>=13
    o['mode']='trajectory';o['steps']=3;o['adaptive']=False
    trajectory=run(p,o)
    assert trajectory['accepted'],[(r['configuration']['name'],r['diagnostic']) for r in trajectory['points'] if not r['accepted']]
    refs=[independent(p,o,trajectory['points'][i]) for i in [0,len(trajectory['points'])//2,-1]]
    high=copy.deepcopy(o);high['mode']='scan';high['configurations']=trajectory['configurations'];high['steps']=2;high['verification_sampling']=33
    verified=run(p,high)
    assert verified['accepted']
    convergence=max(abs(a['rms_um']-b['rms_um']) for a,b in zip(trajectory['points'],verified['points']))
    assert convergence<0.5
    # Known displaced but still valid configurations exercise genuine joint improvement.
    o=settings(m);o['mode']='optimize';o['adaptive']=False;o['require_sensor_coverage']=True
    o['configurations']=copy.deepcopy(m['configurations'])
    for i,c in enumerate(o['configurations']):
        c['positions_mm'][1]+=0.12
        o['variables'].append(dict(kind='position',configuration=i,group=1,min=c['positions_mm'][1]-0.3,max=c['positions_mm'][1]+0.3))
    optimized=run(p,o)
    assert optimized['accepted'],[(r['configuration']['name'],r['diagnostic']) for r in optimized['points'] if not r['accepted']]
    for a,b in zip(optimized['before'],optimized['points']):
        assert all(y<=x+1e-8 for x,y in zip(a['fields_rms_um'],b['fields_rms_um']))
    assert abs(sum(s['thickness'] for s in optimized['reference']['surfaces'])-sum(s['thickness'] for s in p['surfaces']))<1e-9
    long=json.loads((ROOT/'examples/400mm-repaired.oos.json').read_text(encoding='utf8'))
    c=dict(id='focus400',name='400 mm whole lens',positions_mm=[0.],object_distance=None,target_efl_mm=None,target_f_number=None)
    f=settings(m);f.update(mode='focus',configurations=[c],compensator_group=0,steps=3,adaptive=False,require_sensor_coverage=False)
    f['motion'].update(groups=[dict(start=0,end=len(long['surfaces'])-1,min_mm=-25.,max_mm=5.,locked=False)],positions_mm=[0.],min_air_mm=0.,max_rms_um=50.)
    focus=run(long,f)
    assert focus['accepted'],[(r['configuration']['name'],r['diagnostic']) for r in focus['points'] if not r['accepted']]
    focus_ref=independent(long,f,focus['points'][-1])
    # Compare against the unchanged 1.1 keyframe report, not regenerated expectations.
    baseline=json.loads((ROOT/'docs/validation-motion-1.1.json').read_text(encoding='utf8'))
    regression=[]
    for c,b in zip(m['configurations'],baseline['zoom']):
        r=invoke(dict(operation='motion',project=p,motion_options=motion_options(m,c,17)))
        error=abs(r['score_um']-b['rms_um']);assert error<1e-10
        regression.append(error)
    report=dict(version='1.2.0',scan_points=len(scan['points']),scan_passed=scan['accepted'],
                trajectory_points=len(trajectory['points']),trajectory_passed=trajectory['accepted'],
                trajectory_17_to_33_max_rms_change_um=convergence,trajectory_rayoptics=refs,
                optimization_before_rms_um=[r['rms_um'] for r in optimized['before']],
                optimization_after_rms_um=[r['rms_um'] for r in optimized['points']],
                optimization_all_fields_protected=True,focus400=[dict(distance_mm=r['configuration']['object_distance'],positions_mm=r['configuration']['positions_mm'],rms_um=r['rms_um']) for r in focus['points']],
                focus400_rayoptics=focus_ref,legacy_zoom_rms_errors_um=regression,
                scope='Fixed-sensor geometric studies; finite sampled positions only; not continuous parfocality or diffraction certification')
    output=ROOT/(sys.argv[1] if len(sys.argv)>1 else 'docs/validation-study-1.2.json')
    if len(sys.argv)>1: report['version']='1.3.0'
    output.write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n',encoding='utf8')
    if len(sys.argv)==1:
        (ROOT/'docs/fixtures/study-1.2-results.json').write_text(json.dumps(scan,ensure_ascii=False,indent=2)+'\n',encoding='utf8')
    print(json.dumps(report,ensure_ascii=True,indent=2))

if __name__=='__main__':main()
