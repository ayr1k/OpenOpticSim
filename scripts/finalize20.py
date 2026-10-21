"""Record final binary parity and local release evidence, then stage separately."""
import hashlib,json,re,subprocess,shutil
from pathlib import Path
from benchmark20 import difference,invoke
ROOT=Path(__file__).resolve().parents[1]
def read(p):return json.loads((ROOT/p).read_text(encoding='utf-8-sig'))
def sha(p):return hashlib.sha256((ROOT/p).read_bytes()).hexdigest()
benchmark=read('docs/benchmark-2.0.json')
checks=[]
for row in benchmark['results']:
    p=read(f"examples/{row['case']}.oos.json")
    payload=json.dumps(dict(operation=row['operation'],project=p,sampling=33)).encode()
    assert hashlib.sha256(payload).hexdigest()==row['request_sha256']
    a,_=invoke(ROOT/'artifacts/baseline-1.5/oos-cli.exe',payload)
    b,_=invoke(ROOT/'target/release/oos-cli.exe',payload)
    checks.append(dict(case=row['case'],operation=row['operation'],max_absolute_difference=difference(a,b)))
saved=Path.home()/'Downloads/tessar · 原创设计起点.oos.json'
ui=json.loads(saved.read_text(encoding='utf-8'))
assert ui['workspace']['spatial']['options']['poses'][0]['offset_mm'][0]==.1
request=dict(operation='trace_spatial',project=ui,spatial_options=ui['workspace']['spatial']['options'])
trace,_=invoke(ROOT/'target/release/oos-cli.exe',json.dumps(request).encode())
assert all(r['status']=='complete' for r in trace)
assert abs(trace[0]['optical_path_mm']-159.653586)<1e-6
ui_report=dict(browser='local in-app browser with development CLI bridge',saved_and_reopened=True,
    offset_x_mm=.1,complete_rays=len(trace),first_opl_mm=trace[0]['optical_path_mm'],
    screenshot='docs/screenshots/wollaston-2.0.png',saved_project_sha256=hashlib.sha256(saved.read_bytes()).hexdigest(),native_dialog_tested=False)
(ROOT/'docs/validation-ui-2.0.json').write_text(json.dumps(ui_report,indent=2),encoding='utf-8')
total=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',(ROOT/'artifacts/wollaston-independent.log').read_text(encoding='utf-8'))))
report=dict(version='2.0.0',framework_version='1.0.0',date='2026-10-21',frontend_tests=76,rust_desktop_tests=98,
    standalone_framework_tests=total,typescript='passed',production_frontend='passed',clippy_warnings_denied='passed',windows_nsis_build='passed',
    packaged_cli_sha256=sha('target/release/oos-cli.exe'),packaged_dll_sha256=sha('OOS-Wollaston/target/release/wollaston_ffi.dll'),
    final_binary_vs_v15=checks,framework_interfaces=read('docs/validation-framework-2.0.json'),standalone=read('docs/validation-standalone-2.0.json'),
    c_cpp_clients=read('docs/validation-c-2.0.json'),legacy_400mm_samples_unchanged=38508,
    ui=ui_report,clean_windows_install_or_upgrade_tested=False,signed=False,remote_ci_tested=False,
    evidence=['docs/benchmark-2.0.json','docs/validation-study-2.0.json','docs/validation-diffraction-2.0.json','docs/validation-design-saved-1.0.json'],
    limitations=['3D explicit-ray geometry only; existing quality/optimization/PDF remain axisymmetric.',
                'No GPU, nonsequential paths, spatial pupil aiming, spatial MTF or automatic 3D collision certification.',
                'Benchmark includes CLI startup/JSON and does not imply all-hardware speedup.'])
(ROOT/'docs/validation-release-2.0.json').write_text(json.dumps(report,indent=2,ensure_ascii=False),encoding='utf-8')
for name in ['benchmark-2.0.json','validation-framework-2.0.json','validation-standalone-2.0.json','validation-c-2.0.json']:
    shutil.copy2(ROOT/'docs'/name,ROOT/'OOS-Wollaston/docs'/name)
print(json.dumps(dict(final_binary_comparisons=len(checks),frontend_tests=76,framework_tests=total,ui_complete_rays=len(trace))))
