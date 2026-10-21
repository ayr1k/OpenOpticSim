"""Collect the independent framework's own locked dependency notices."""
import json,subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];W=ROOT/'OOS-Wollaston'
meta=json.loads(subprocess.check_output(['cargo','metadata','--manifest-path',str(W/'Cargo.toml'),'--locked','--offline','--format-version','1'],encoding='utf-8'))
records=[];blocks=['OOS-Wollaston third-party notices. Includes build dependencies.\n']
for p in sorted(meta['packages'],key=lambda p:p['name']):
    if p['source'] is None:continue
    folder=Path(p['manifest_path']).parent
    files=[f for f in folder.iterdir() if f.is_file() and f.name.upper().startswith(('LICENSE','COPYING','NOTICE','COPYRIGHT'))]
    if p.get('license_file') and folder/p['license_file'] not in files:files.append(folder/p['license_file'])
    assert files and p.get('license'),p['name']
    source=f"https://crates.io/crates/{p['name']}/{p['version']}"
    blocks.append(f"\n{'='*60}\n{p['name']} {p['version']}\n{p['license']}\n{source}\n")
    for f in files:blocks.append(f'\n--- {f.name} ---\n'+f.read_text(encoding='utf-8'))
    records.append(dict(name=p['name'],version=p['version'],license=p['license'],source=source,texts=[f.name for f in files]))
(W/'THIRD_PARTY_NOTICES.txt').write_text('\n'.join(blocks),encoding='utf-8')
(W/'docs/dependencies.json').write_text(json.dumps(records,indent=2),encoding='utf-8')
for source,dest in [('docs/catalog/CC0-1.0.txt','data/CC0-1.0.txt'),('docs/DIFFRACTION.md','docs/DIFFRACTION.md')]:
    (W/dest).write_bytes((ROOT/source).read_bytes())
(W/'NOTICE').write_text('''OOS-Wollaston
Copyright 2026 OpenOpticSim contributors

Extracted from OpenOpticSim. Original code and synthetic teaching prescriptions: Apache-2.0.
The SCHOTT glass subset in data/glass-catalog.json comes from the CC0-1.0 refractiveindex.info database by Mikhail Polyanskiy.
Original data references: SCHOTT catalog 2017-01-20b. Provenance and checksums are in that JSON; license in data/CC0-1.0.txt.
M. N. Polyanskiy, Scientific Data 11, 94 (2024), https://doi.org/10.1038/s41597-023-02898-2
RayOptics is an independent development reference, not the implementation or a runtime dependency.
Examples are teaching designs, not official commercial prescriptions.
Dependency licenses and upstream source URLs are in THIRD_PARTY_NOTICES.txt and docs/dependencies.json.
''',encoding='utf-8')
print(f'{len(records)} independent dependency notices')
