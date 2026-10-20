"""Verify local resources and stage versioned assets. Never commits, tags or uploads."""
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import tomllib
import zipfile

ROOT = Path(__file__).resolve().parents[1]


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def read(path):
    return json.loads(path.read_text(encoding='utf-8-sig'))


def main():
    package = read(ROOT / 'package.json')
    config = read(ROOT / 'src-tauri/tauri.conf.json')
    version = package['version']
    cargo = tomllib.loads((ROOT / 'Cargo.toml').read_text(encoding='utf-8'))
    if version not in ('1.0.0', '1.1.0', '1.2.0', '1.3.0') or config['version'] != version or cargo['workspace']['package']['version'] != version:
        raise SystemExit('This staging recipe is for matching supported source and desktop versions only.')
    installer = ROOT / f'target/release/bundle/nsis/OpenOpticSim_{version}_x64-setup.exe'
    executable = ROOT / 'target/release/openopticsim-desktop.exe'
    if not installer.is_file() or not executable.is_file():
        raise SystemExit('Build the NSIS installer first: pnpm release:build')
    with installer.open('rb') as stream:
        if stream.read(2) != b'MZ':
            raise SystemExit('Installer is not a Windows executable.')
    files = subprocess.check_output(['git', 'ls-files', '--cached', '--others', '--exclude-standard', '-z'], cwd=ROOT)
    candidates = {p.decode() for p in files.split(b'\0') if p}
    binary_checks = []
    for binary in [executable, ROOT / 'target/release/oos-cli.exe']:
        data = binary.read_bytes()
        ascii_count = len(re.findall(rb'(?i)[A-Z]:[\\/]+Users[\\/]+[^\\/\x00\r\n]+', data))
        utf16_count = len(re.findall(r'(?i)[A-Z]:[\\/]+Users[\\/]+[^\\/\x00\r\n]+', data.decode('utf-16-le', errors='ignore')))
        binary_checks.append({'name': binary.name, 'personal_path_ascii_matches': ascii_count,
                              'personal_path_utf16_matches': utf16_count, 'sha256': digest(binary)})
        if ascii_count or utf16_count:
            raise SystemExit(f'Personal build path remains in {binary.name}; use pnpm release:build.')

    # Refuse a bundle older than its explicit inputs. This catches accidental
    # staging of a previous installer after changing notices / documentation.
    resources = []
    bundle_inputs = [ROOT / 'src-tauri/tauri.conf.json', executable]
    source_roots = ('src/', 'crates/', 'src-tauri/')
    source_files = {'Cargo.toml', 'Cargo.lock', 'package.json', 'pnpm-lock.yaml',
                    'pnpm-workspace.yaml', 'vite.config.ts', 'tsconfig.json',
                    'index.html', 'scripts/build_manual.mjs'}
    for name in sorted(candidates):
        if name.startswith(source_roots) or name in source_files:
            if not (ROOT / name).is_file():
                raise SystemExit(f'Build input was removed; rebuild before staging: {name}')
            bundle_inputs.append(ROOT / name)
    for source, destination in config['bundle']['resources'].items():
        path = ROOT / 'src-tauri' / source
        files = sorted(path.rglob('*')) if path.is_dir() else [path]
        for item in files:
            if not item.is_file():
                continue
            relative = Path(destination) / item.relative_to(path) if path.is_dir() else Path(destination)
            bundled = ROOT / 'target/release' / relative
            if not bundled.is_file() or digest(item) != digest(bundled):
                raise SystemExit(f'Resource missing or stale; rebuild installer: {relative.as_posix()}')
            resources.append({'name': relative.as_posix(), 'sha256': digest(item), 'bundled_matches': True})
            bundle_inputs.append(item)
    if installer.stat().st_mtime + 2 < max(path.stat().st_mtime for path in bundle_inputs):
        raise SystemExit('Installer predates a bundle input; rebuild instead of relabeling an old installer.')
    font_meta = read(ROOT / 'src/assets/fonts/source.json')
    if digest(ROOT / 'src/assets/fonts/NotoSansSC.ttf') != font_meta['sha256']:
        raise SystemExit('Bundled font differs from the documented asset hash.')
    dependencies = read(ROOT / 'docs/dependencies.json')
    if any(not row.get('license') or not row.get('texts') or not row.get('source') for row in dependencies):
        raise SystemExit('Review missing dependency attribution before staging.')
    forbidden = [p for p in candidates if p.startswith(('target/', 'node_modules/', 'output/', 'artifacts/', '.tools/', 'tmp/'))]
    if forbidden:
        raise SystemExit('Generated/private directories are publication candidates; inspect Git ignore rules.')
    revision = subprocess.run(['git', 'rev-parse', '--verify', 'HEAD'], cwd=ROOT, capture_output=True, text=True)
    source_commit = revision.stdout.strip() if revision.returncode == 0 else None
    status = subprocess.check_output(['git', 'status', '--porcelain'], cwd=ROOT)
    series = '.'.join(version.split('.')[:2])
    release_path = ROOT / f'docs/release-{series}.json'
    release = read(release_path) if release_path.exists() else {}
    validation_path = ROOT / f'docs/validation-release-{series}.json'
    if validation_path.exists():
        release['validation'] = read(validation_path)
    startup_path = ROOT / f'artifacts/startup-release-{series}.json'
    if not startup_path.exists():
        startup_path = ROOT / f'docs/startup-{series}.json'
    startup = read(startup_path) if startup_path.exists() else None
    executable_hash = digest(executable)
    if not startup or startup.get('executable_sha256', '').lower() != executable_hash or startup.get('exited') is not False or startup.get('responding') is not True:
        startup = None  # Never attach an older binary's smoke check to a new one.
    if startup and 'installer_wizard_startup' in startup:
        wizard = startup['installer_wizard_startup']
        if wizard.get('installer_sha256', '').lower() != digest(installer):
            startup.pop('installer_wizard_startup')
    release.update({'version': version, 'status': 'prepared_for_owner_publication',
                    'installer': installer.relative_to(ROOT).as_posix(),
                    'bytes': installer.stat().st_size, 'sha256': digest(installer),
                    'executable_sha256': executable_hash, 'resources': resources,
                    'dependency_records': len(dependencies), 'startup': startup,
                    'binary_privacy_checks': binary_checks,
                    'source_commit': source_commit, 'source_worktree_dirty': bool(status.strip()),
                    'license_sha256': digest(ROOT / 'LICENSE'),
                    'remote_ci_run': None, 'remote_ci_status': 'not_checked_by_local_staging'})
    release['installer_wizard_startup_tested'] = bool(startup and startup.get('installer_wizard_startup', {}).get('expected_window_found'))
    release.pop('superseded_by', None)

    out = ROOT / 'artifacts' / f'release-{version}'
    out.mkdir(parents=True, exist_ok=True)
    if out.is_symlink() or not out.resolve().is_relative_to(ROOT.resolve()):
        raise SystemExit('Release staging must stay inside the workspace.')
    names = []

    def output_path(name):
        dest = out / name
        if dest.is_symlink() or not dest.resolve().is_relative_to(out.resolve()):
            raise SystemExit(f'Refusing unsafe output path: {name}')
        return dest

    def copy(source, name):
        dest = output_path(name)
        shutil.copy2(source, dest)
        names.append(name)

    # Generated reports are Release assets, not tracked source files. Keeping
    # them outside docs avoids a commit -> report -> new commit provenance loop.
    output_path('build-report.json').write_text(json.dumps(release, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    names.append('build-report.json')
    copy(installer, installer.name)
    checksum_name = installer.name + '.sha256'
    checksum = f'{digest(installer)}  {installer.name}\n'
    output_path(checksum_name).write_text(checksum, encoding='ascii')
    sidecar = installer.parent / checksum_name
    if sidecar.is_symlink():
        raise SystemExit('Refusing installer checksum symlink.')
    sidecar.write_text(checksum, encoding='ascii')
    names.append(checksum_name)
    copy(ROOT / f'docs/OOS-{series}-使用手册.html', f'OpenOpticSim-{version}-manual.html')
    copy(ROOT / f'docs/releases/{version}.md', f'OpenOpticSim-{version}-release-notes.md')
    for name in ['LICENSE', 'NOTICE', 'THIRD_PARTY_NOTICES.txt']:
        copy(ROOT / name, name)

    archive_name = f'OpenOpticSim-{version}-docs-and-examples.zip'
    archive_files = sorted(p for p in candidates if p.startswith(('docs/', 'examples/')))
    archive_files += ['README.md', 'CONTRIBUTING.md', 'CHANGELOG.md', 'SECURITY.md',
                      'LICENSE', 'NOTICE', 'THIRD_PARTY_NOTICES.txt',
                      'src/assets/fonts/OFL.txt', 'src/assets/fonts/source.json', 'src/data/glass-catalog.json']
    with zipfile.ZipFile(output_path(archive_name), 'w', zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
        for name in sorted(set(archive_files)):
            entry = zipfile.ZipInfo(name, date_time=(2026, 10, 19, 0, 0, 0))
            entry.compress_type = zipfile.ZIP_DEFLATED
            archive.writestr(entry, (ROOT / name).read_bytes())
    names.append(archive_name)

    manifest = {'version': version, 'status': 'prepared_for_owner_publication', 'source_commit': source_commit,
                'source_commit_required_before_publication': source_commit is None or bool(status.strip()),
                'tag_source_commit': source_commit if not status.strip() else None,
                'source_worktree_dirty': bool(status.strip()),
                'source_snapshot_finalized': source_commit is not None and not status.strip(),
                'signed': False, 'clean_windows_offline_install_tested': False,
                'native_pdf_dialog_end_to_end_tested': False, 'remote_ci_run': None,
                'remote_ci_status': 'not_checked_by_local_staging',
                'license_sha256': digest(ROOT / 'LICENSE'),
                'resources': resources, 'dependency_records': len(dependencies),
                'binary_privacy_checks': binary_checks,
                'font_asset_sha256': font_meta['sha256'],
                'installer_startup': startup,
                'assets': [{'name': name, 'bytes': (out / name).stat().st_size,
                            'sha256': digest(out / name)} for name in sorted(names)],
                'notes': ['This tool does not commit, push, create tags or publish a GitHub Release.',
                          'When the worktree is dirty, source_commit is the base commit, not the complete prepared source.',
                          'Commit prepared source changes in GitHub Desktop, then rerun staging to bind the final commit.',
                          'Rebuild if optical source, configuration, dependencies or bundled resources changed.']}
    output_path('manifest.json').write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    names.append('manifest.json')
    sums = ''.join(f'{digest(out / name)}  {name}\n' for name in sorted(names))
    output_path('SHA256SUMS.txt').write_text(sums, encoding='utf-8')
    print(json.dumps({'version': version, 'directory': out.relative_to(ROOT).as_posix(),
                      'assets': len(names) + 1, 'resources_verified': len(resources),
                      'source_commit': source_commit, 'published': False}))


if __name__ == '__main__':
    main()
