"""Same-process-launch, same-request comparison against frozen v1.5 CLI."""
import hashlib,json,platform,statistics,subprocess,time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
old=ROOT/'artifacts/baseline-1.5/oos-cli.exe'
new=ROOT/'target/release/oos-cli.exe'
def digest(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def invoke(exe,payload):
    start=time.perf_counter()
    r=subprocess.run([str(exe),'--request'],input=payload,capture_output=True)
    elapsed=time.perf_counter()-start
    if r.returncode:raise RuntimeError(r.stderr.decode())
    return json.loads(r.stdout),elapsed
def difference(a,b):
    if isinstance(a,dict):
        assert a.keys()==b.keys()
        return max((difference(a[k],b[k]) for k in a),default=0.)
    if isinstance(a,list):
        assert len(a)==len(b)
        return max((difference(x,y) for x,y in zip(a,b)),default=0.)
    if isinstance(a,(int,float)) and not isinstance(a,bool):
        assert isinstance(b,(int,float))
        delta=abs(a-b);assert delta<=1e-9*max(1,abs(a)),(a,b)
        return delta
    assert a==b,(a,b)
    return 0.
def main():
    rows=[]
    for name in ['tessar-50mm-1.3','planar-50mm-1.3','double_gauss-50mm-1.3','sonnar-50mm-1.3','24-70mm-zoom','400mm-repaired']:
        path=ROOT/'examples'/f'{name}.oos.json';p=json.loads(path.read_text(encoding='utf-8'))
        operations=['analyze']+(['wave','preview'] if name in ['tessar-50mm-1.3','400mm-repaired'] else [])
        for op in operations:
            request=dict(operation=op,project=p,sampling=33)
            payload=json.dumps(request).encode();a,_=invoke(old,payload);b,_=invoke(new,payload)
            delta=difference(a,b);before=[];after=[]
            for i in range(5):
                for exe,times in ([(old,before),(new,after)] if i%2==0 else [(new,after),(old,before)]):
                    _,elapsed=invoke(exe,payload);times.append(elapsed)
            row=dict(case=name,operation=op,source_sha256=digest(path),request_sha256=hashlib.sha256(payload).hexdigest(),max_absolute_difference=delta,
                baseline_seconds=before,current_seconds=after,speedup=statistics.median(before)/statistics.median(after))
            rows.append(row);print(name,op,round(row['speedup'],3),'x',flush=True)
    report=dict(baseline_sha256=digest(old),current_sha256=digest(new),platform=platform.platform(),processor=platform.processor(),
        method='One warmup, five interleaved repetitions; median wall time includes CLI startup and JSON serialization. Same requests, double precision, no relaxed tolerances. No concurrent benchmark workloads.',results=rows)
    (ROOT/'docs/benchmark-2.0.json').write_text(json.dumps(report,indent=2),encoding='utf-8')
if __name__=='__main__':main()
