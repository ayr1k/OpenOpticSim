"""Build/test a copy outside the parent checkout; do not reuse parent target."""
import json,os,shutil,subprocess,tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];W=ROOT/'OOS-Wollaston'
folder=Path(tempfile.mkdtemp(prefix='oos-wollaston-independent-'))
dest=folder/'OOS-Wollaston'
shutil.copytree(W,dest,ignore=shutil.ignore_patterns('target','.git','__pycache__'))
env=os.environ.copy();env.pop('CARGO_TARGET_DIR',None)
env['PATH']=str(Path.home()/'.cargo/bin')+os.pathsep+env['PATH']
cargo=str(Path.home()/'.cargo/bin/cargo.exe')
log=ROOT/'artifacts/wollaston-independent.log'
with log.open('w',encoding='utf-8') as out:
    subprocess.run([cargo,'test','--workspace','--locked','--offline','--release'],cwd=dest,env=env,stdout=out,stderr=subprocess.STDOUT,check=True)
    subprocess.run([cargo,'build','--workspace','--locked','--offline','--release'],cwd=dest,env=env,stdout=out,stderr=subprocess.STDOUT,check=True)
request=dict(operation='analyze',project=json.loads((dest/'examples/singlet.oos.json').read_text(encoding='utf-8')),sampling=7)
result=subprocess.run([str(dest/'target/release/wollaston-cli.exe')],input=json.dumps(request),capture_output=True,text=True,check=True)
assert 'paraxial' in json.loads(result.stdout)
report=dict(outside_parent_checkout=True,locked_offline_build=True,full_framework_tests_passed=True,standalone_cli_analyze=True,used_parent_target=False,
            source_folder='OOS-Wollaston',temporary_copy_retained=True)
(ROOT/'docs/validation-standalone-2.0.json').write_text(json.dumps(report,indent=2),encoding='utf-8')
print(json.dumps(report))
