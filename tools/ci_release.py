#!/usr/bin/env python3
"""Build and verify release archives; publication is handled by GitHub Actions."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import zipfile

ROOT = Path(__file__).resolve().parents[1]
VERSION = re.compile(r'v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?')


def release_version(event, ref, commit, run_id):
    if event == 'push':
        match = VERSION.fullmatch(ref)
        if not match or (match[4] and any(p.isdigit() and len(p) > 1 and p.startswith('0') for p in match[4].split('.'))):
            raise ValueError('Release tag must be vMAJOR.MINOR.PATCH with an optional SemVer prerelease')
        return ref[1:]
    if event == 'workflow_dispatch':
        return f'test-{commit[:12]}-{run_id}'
    raise ValueError('Unsupported workflow event')


def verify_archives(output, version):
    stem = output / ('weather-forecast-' + version)
    with zipfile.ZipFile(str(stem) + '.zip') as archive:
        zipped = {n: archive.read(n) for n in archive.namelist() if not n.endswith('/')}
    with tarfile.open(str(stem) + '.tar.gz') as archive:
        tarred = {m.name: archive.extractfile(m).read() for m in archive.getmembers() if m.isfile()}
    if zipped != tarred:
        raise ValueError('Release archive payloads differ')
    base = stem.name + '/'
    sums = zipped[base + 'SHA256SUMS'].decode().splitlines()
    expected = set(zipped) - {base + 'SHA256SUMS'}
    covered = set()
    for line in sums:
        digest, name = line.split('  ', 1)
        if hashlib.sha256(zipped[base + name]).hexdigest() != digest:
            raise ValueError('Invalid payload checksum: ' + name)
        covered.add(base + name)
    if covered != expected:
        raise ValueError('Incomplete payload checksums')
    paths = [Path(str(stem) + suffix) for suffix in ('.zip', '.tar.gz')]
    (output / 'SHA256SUMS').write_text(''.join(f'{hashlib.sha256(p.read_bytes()).hexdigest()}  {p.name}\n' for p in paths))


def build(version, output):
    before = subprocess.check_output(['git', 'diff'], cwd=ROOT)
    status = subprocess.check_output(['git', 'status', '--porcelain'], cwd=ROOT)
    command = [str(ROOT / 'build.sh'), '--locked', '--message-format=json-render-diagnostics']
    process = subprocess.Popen(command, cwd=ROOT, stdout=subprocess.PIPE, text=True)
    build_dirs, executables = set(), set()
    for line in process.stdout:
        try:
            message = json.loads(line)
        except json.JSONDecodeError:
            print(line, end='', flush=True)
            continue
        if message.get('reason') == 'compiler-message':
            print(message['message'].get('rendered', ''), end='', flush=True)
        if message.get('reason') == 'build-script-executed' and 'esp-idf-sys' in message.get('package_id', ''):
            build_dirs.add(Path(message['out_dir']) / 'build')
        if message.get('reason') == 'compiler-artifact' and message.get('target', {}).get('name') == 'weather-forecast-firmware' and message.get('executable'):
            executables.add(Path(message['executable']))
    if process.wait() != 0:
        raise ValueError('Firmware build failed')
    if len(build_dirs) != 1 or len(executables) != 1:
        raise ValueError('Cannot uniquely identify current SDK build and application ELF')
    idf_build, elf = build_dirs.pop(), executables.pop()
    project = json.loads((idf_build / 'project_description.json').read_text())
    compiler = Path(project['c_compiler']).resolve()
    toolchain = compiler.parent.parent
    sdk = Path(project['idf_path']).resolve()
    pythons = list((sdk.parent.parent / 'python_env').glob('*/bin/python'))
    if len(pythons) != 1:
        raise ValueError('Cannot uniquely identify ESP-IDF esptool Python environment')
    python = pythons[0]
    with tempfile.TemporaryDirectory(prefix='weather-ci-') as temp:
        temp = Path(temp)
        images = temp / 'images'
        images.mkdir()
        shutil.copyfile(elf, images / 'weather-forecast-firmware')
        shutil.copyfile(idf_build / 'bootloader/bootloader.bin', images / 'bootloader.bin')
        shutil.copyfile(idf_build / 'partition_table/partition-table.bin', images / 'partition-table.bin')
        subprocess.run([str(python), '-m', 'esptool', '--chip', 'esp32', 'elf2image', '--flash_mode', 'dio', '--flash_freq', '40m', '--flash_size', '4MB', '--output', str(images / 'weather-forecast-firmware.bin'), str(elf)], check=True)
        common = ['--idf-build-dir', str(idf_build), '--toolchain-dir', str(toolchain), '--firmware-dir', str(images), '--collection-dir', str(temp / 'licenses')]
        tool = [sys.executable, str(ROOT / 'tools/license_release.py')]
        subprocess.run(tool + ['update'] + common, check=True)
        subprocess.run(tool + ['package'] + common + ['--version', version, '--output-dir', str(output), '--esptool-python', str(python)], check=True)
    verify_archives(output, version)
    if subprocess.check_output(['git', 'diff'], cwd=ROOT) != before or subprocess.check_output(['git', 'status', '--porcelain'], cwd=ROOT) != status:
        raise ValueError('Build or evidence collection changed repository files')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=['version', 'build'])
    parser.add_argument('--output-dir', type=Path, default=ROOT / 'firmware/target/license-release')
    args = parser.parse_args()
    version = release_version(os.environ['GITHUB_EVENT_NAME'], os.environ['GITHUB_REF_NAME'], os.environ['GITHUB_SHA'], os.environ['GITHUB_RUN_ID'])
    if args.command == 'version':
        with open(os.environ['GITHUB_OUTPUT'], 'a') as stream:
            stream.write(f'version={version}\nprerelease={str("-" in version).lower()}\n')
    else:
        build(version, args.output_dir.resolve())


if __name__ == '__main__':
    main()
