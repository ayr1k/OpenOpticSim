"""CLI/C ABI/Python parity and independent transformed-ray reference."""
import json,sys,subprocess,hashlib
from pathlib import Path
import numpy as np
from rayoptics.elem.profiles import Spherical,EvenPolynomial
from rayoptics.raytr.raytrace import bend
ROOT=Path(__file__).resolve().parents[1];W=ROOT/'OOS-Wollaston'
sys.path.insert(0,str(W/'python'))
from wollaston import Wollaston
from validate_rayoptics import index
def main():
    engine=Wollaston(W/'target/release/wollaston_ffi.dll')
    p=json.loads((ROOT/'examples/tessar-50mm-1.3.oos.json').read_text(encoding='utf-8'))
    req=dict(operation='analyze',project=p,sampling=17)
    c=engine.execute(req)
    r=subprocess.run([str(W/'target/release/wollaston-cli.exe')],input=json.dumps(req),text=True,capture_output=True,check=True)
    assert c==json.loads(r.stdout)
    # Arbitrary small XYZ offsets and rotations, skew ray; independent RayOptics intersections.
    angles=[.003,-.005,.002];rx,ry,rz=angles
    Rx=np.array([[1,0,0],[0,np.cos(rx),-np.sin(rx)],[0,np.sin(rx),np.cos(rx)]])
    Ry=np.array([[np.cos(ry),0,np.sin(ry)],[0,1,0],[-np.sin(ry),0,np.cos(ry)]])
    Rz=np.array([[np.cos(rz),-np.sin(rz),0],[np.sin(rz),np.cos(rz),0],[0,0,1]])
    R=Rz@Ry@Rx
    spatial=dict(poses=[dict(offset_mm=[.01,-.02,.005],rotation_rad=angles) for _ in p['surfaces']],
                 rays=[dict(origin_mm=[.1,.2,-10],direction=[.001,.002,1],wavelength_nm=587.5618)],threads=4)
    request=dict(operation='trace_spatial',project=p,spatial_options=spatial)
    ray=engine.execute(request)[0];assert ray['status']=='complete',ray
    q=np.array(spatial['rays'][0]['origin_mm']);d=np.array(spatial['rays'][0]['direction']);d/=np.linalg.norm(d)
    z=0.;n=1.;opl=0.;errors=[]
    for s,h in zip(p['surfaces'],ray['hits']):
        vertex=np.array([.01,-.02,z+.005]);local=R.T@(q-vertex);ld=R.T@d
        profile=EvenPolynomial(c=s['curvature'],cc=s['conic'],coefs=[0.,*s['asphere']]) if s['conic'] or any(s['asphere']) else Spherical(c=s['curvature'])
        distance,hit=profile.intersect(local,ld,eps=1e-12,z_dir=1);normal=profile.normal(hit)
        n2=index(p['materials'][s['material']],587.5618)
        outgoing=bend(ld,normal,n,n2)
        q=vertex+R@hit;d=R@outgoing;opl+=n*distance
        errors.append(float(np.linalg.norm(q-np.array(h['position_mm']))));assert errors[-1]<1e-6
        assert abs(opl-h['optical_path_mm'])<1e-6
        assert np.linalg.norm(d-np.array(h['outgoing']))<1e-8
        z+=s['thickness'];n=n2
    with engine.cancellation() as token:
        token.cancel()
        try:engine.execute(request,token)
        except RuntimeError as e:assert 'cancel' in str(e)
        else:raise AssertionError('cancel ignored')
    try:engine.execute(dict(operation='wave',project=p,spatial_options=spatial))
    except RuntimeError as e:assert 'only supported' in str(e)
    else:raise AssertionError('spatial configuration silently ignored')
    for _ in range(100):
        try:engine.execute({})
        except RuntimeError:pass
        else:raise AssertionError('malformed input accepted')
    report=dict(cli_python_cabi_identical=True,independent_reference='RayOptics profiles + bend with NumPy frame transforms',
                transformed_surfaces=len(errors),max_hit_error_mm=max(errors),cancelled=True,malformed_requests=100,
                spatial_options_rejected_for_wave=True,dll_sha256=hashlib.sha256((W/'target/release/wollaston_ffi.dll').read_bytes()).hexdigest())
    (ROOT/'docs/validation-framework-2.0.json').write_text(json.dumps(report,indent=2),encoding='utf-8')
    print(json.dumps(report))
if __name__=='__main__':main()
