"""Rebuild the small offline catalog from a pinned CC0 refractiveindex.info snapshot."""
import hashlib
import json
from pathlib import Path
import re
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
DEST = ROOT / "src/data/glass-catalog.json"
REPO = "polyanskiy/refractiveindex.info-database"
headers = {"User-Agent": "OpenOpticSim-catalog-import"}

def fetch(url):
    return urllib.request.urlopen(urllib.request.Request(url, headers=headers), timeout=45).read()

if DEST.exists():
    commit = json.loads(DEST.read_text(encoding="utf-8"))["commit"]
else:
    commit = json.loads(fetch(f"https://api.github.com/repos/{REPO}/commits/main"))["sha"]
entries = []
for name in ["N-BK7", "N-BAK4", "N-SK16", "N-LAK22", "N-F2", "N-SF5", "N-SF10", "N-SF11", "N-BAF4", "N-BAF10", "N-BAK1", "N-BALF4", "N-FK5", "N-FK51A", "N-LAK8", "N-LAK10", "N-LAK12", "N-LAK14", "N-LAK21", "N-LASF9", "N-LASF31A", "N-LASF44", "N-PSK3", "N-PSK53A", "N-SF1", "N-SF2", "N-SF4", "N-SF6", "N-SF8", "N-SK2", "N-SK4", "N-SK5", "N-SSK2"]:
    path = f"database/data/specs/schott/optical/{name}.yml"
    url = f"https://raw.githubusercontent.com/{REPO}/{commit}/{path}"
    raw = fetch(url)
    text = raw.decode("utf-8")
    block = re.search(r"type: formula 2\s+wavelength_range: ([^\n]+)\s+coefficients: ([^\n]+)", text)
    assert block, name
    range_um = list(map(float, block[1].split()))
    c = list(map(float, block[2].split()))
    assert len(c) == 7 and c[0] == 0
    nd = float(re.search(r"\n\s+nd: ([^\n]+)", text)[1])
    vd = float(re.search(r"\n\s+Vd: ([^\n]+)", text)[1])
    entries.append(dict(name=f"SCHOTT {name}", n=nd, sellmeier=[[c[1],c[3],c[5]],[c[2],c[4],c[6]]],
                        range_nm=[x*1000 for x in range_um],
                        source=f"refractiveindex.info CC0-1.0; SCHOTT catalog 2017-01-20b; 293 K; {url}; SHA256 {hashlib.sha256(raw).hexdigest()}",
                        nd_reference=nd, vd_reference=vd, url=url, sha256=hashlib.sha256(raw).hexdigest()))
DEST.parent.mkdir(exist_ok=True)
DEST.write_text(json.dumps(dict(commit=commit,license="CC0-1.0",retrieved="2026-10-20",entries=entries),indent=2)+"\n", encoding="utf-8")
notice = ROOT / "docs/catalog"
notice.mkdir(exist_ok=True)
(notice/"CC0-1.0.txt").write_bytes(fetch(f"https://raw.githubusercontent.com/{REPO}/{commit}/LICENSE"))
print(json.dumps(dict(commit=commit,glasses=len(entries))))
