"""Stage independent source/SDK assets without committing or publishing."""
import hashlib,json,subprocess,zipfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];W=ROOT/'OOS-Wollaston';OUT=ROOT/'artifacts/wollaston-1.0.0'
OUT.mkdir(exist_ok=True,parents=True)
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
sources=[p for p in W.rglob('*') if p.is_file() and not any(x in ('target','.git','__pycache__') for x in p.relative_to(W).parts)]
def archive(name,entries):
    with zipfile.ZipFile(OUT/name,'w',zipfile.ZIP_DEFLATED) as z:
        for dest,source in sorted(entries):z.write(source,dest)
archive('OOS-Wollaston-1.0.0-source.zip',[(p.relative_to(W).as_posix(),p) for p in sources])
sdk=[(p.relative_to(W).as_posix(),p) for p in sources if p.relative_to(W).parts[0] in ('docs','include','python','examples','data') or p.name in ('README.md','LICENSE','NOTICE','THIRD_PARTY_NOTICES.txt')]
for name in ['wollaston_ffi.dll','wollaston_ffi.dll.lib','wollaston-cli.exe']:
    p=W/'target/release'/name;assert p.is_file(),name;sdk.append(('bin/'+name,p))
archive('OOS-Wollaston-1.0.0-windows-x64-sdk.zip',sdk)
(OUT/'OOS-Wollaston-1.0.0-release-notes.md').write_bytes((W/'docs/RELEASE-1.0.0.md').read_bytes())
report=dict(version='1.0.0',desktop_version='2.0.0',abi=1,signed=False,published=False,
            source_base_commit=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),
            source_worktree_dirty=bool(subprocess.check_output(['git','status','--porcelain'],cwd=ROOT).strip()),
            source_files=[dict(path=p.relative_to(W).as_posix(),sha256=sha(p)) for p in sorted(sources)],
            evidence={name:json.loads((ROOT/'docs'/name).read_text(encoding='utf-8')) for name in ['validation-framework-2.0.json','validation-standalone-2.0.json','validation-c-2.0.json']},
            assets=[dict(name=p.name,bytes=p.stat().st_size,sha256=sha(p)) for p in sorted(OUT.iterdir()) if p.suffix in ('.zip','.md')])
(OUT/'manifest.json').write_text(json.dumps(report,indent=2,ensure_ascii=False),encoding='utf-8')
(OUT/'SHA256SUMS.txt').write_text(''.join(f'{sha(p)}  {p.name}\n' for p in sorted(OUT.iterdir()) if p.name!='SHA256SUMS.txt'),encoding='utf-8')
for p in OUT.glob('*.zip'):
    with zipfile.ZipFile(p) as z:assert z.testzip() is None
print(json.dumps(dict(directory=OUT.relative_to(ROOT).as_posix(),assets=len(list(OUT.iterdir())),source_files=len(sources))))
