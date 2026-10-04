#!/usr/bin/env python3
"""Collect pinned license evidence, check it offline, and package ESP32 releases.

Checks validate evidence coverage and integrity, not legal compliance or ownership.
"""
import argparse
import concurrent.futures
import csv
import hashlib
from html.parser import HTMLParser
import io
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import urllib.error
import urllib.request
import zipfile

ROOT = Path(__file__).resolve().parents[1]
COLLECTION = ROOT / 'docs/licensing'
INVENTORY = ROOT / 'docs/third-party-rust-2026-10-04.csv'
POLICY = ROOT / 'docs/licensing-policy.json'
SDK_INVENTORY = ROOT / 'docs/third-party-esp-idf-2026-10-04.json'
IMAGES = ('bootloader.bin', 'partition-table.bin', 'weather-forecast-firmware.bin')
INPUTS = ('LICENSE', 'firmware/Cargo.lock', 'firmware/Cargo.toml',
          'firmware/sdkconfig.defaults', 'firmware/partitions.csv', 'build.sh',
          'docs/third-party-rust-2026-10-04.csv', 'docs/third-party-esp-idf-2026-10-04.json', 'docs/licensing-policy.json')
ALLOWED = {'MIT', 'MIT AND Unicode-3.0', 'Apache-2.0', 'BSD-3-Clause', 'Zlib', 'ISC'}


def digest(data):
    return hashlib.sha256(data).hexdigest()


def require(condition, message):
    if not condition:
        raise ValueError(message)


def run(*args):
    return subprocess.check_output([str(a) for a in args], text=True).strip()


def rustup():
    return shutil.which('rustup') or str(Path.home() / '.cargo/bin/rustup')


def rust_root():
    return Path(run(rustup(), 'which', '--toolchain', 'esp', 'rustc')).resolve().parents[1]


def rust_version():
    return run(rustup(), 'run', 'esp', 'rustc', '-vV')


def fetch(url):
    with urllib.request.urlopen(urllib.request.Request(url, headers={
            'User-Agent': 'weather-firmware-license-collector'}), timeout=45) as response:
        return response.read()


def notice_name(name):
    base = Path(name).name.lower()
    return base.startswith(('license', 'licence', 'copying', 'copyright', 'notice', 'authors'))


def rows():
    with INVENTORY.open() as source:
        return list(csv.DictReader(source))


def lock_packages():
    parts = (ROOT / 'firmware/Cargo.lock').read_text().split('[[package]]')[1:]
    return {(re.search(r'^name = "([^"]+)"', p, re.M)[1],
             re.search(r'^version = "([^"]+)"', p, re.M)[1])
            for p in parts if re.search(r'^source = ', p, re.M)}


def target_packages():
    metadata = json.loads(run(rustup(), 'run', 'stable', 'cargo', 'metadata',
        '--manifest-path', ROOT / 'firmware/Cargo.toml', '--locked', '--offline',
        '--filter-platform', 'xtensa-esp32-espidf', '--format-version', '1'))
    packages = {p['id']: p for p in metadata['packages']}
    nodes = {n['id']: n for n in metadata['resolve']['nodes']}
    root = metadata['resolve']['root']
    result = {}
    excluded = json.loads(POLICY.read_text()).get('excluded_codegen', {})
    def visit(identifier):
        if identifier in result:
            return
        package = packages[identifier]
        if package['name'] + '@' + package['version'] in excluded:
            return
        macro = any('proc-macro' in t['kind'] for t in package['targets'])
        result[identifier] = {'name': package['name'], 'version': package['version'],
            'coverage': 'code-generation notice' if macro else 'ESP32 target dependency'}
        # Keep notices for macro providers conservatively, without their host helpers.
        if not macro:
            for dependency in nodes[identifier]['deps']:
                if any(kind['kind'] is None for kind in dependency['dep_kinds']):
                    visit(dependency['pkg'])
    visit(root)
    return sorted([v for k, v in result.items() if k != root], key=lambda v: (v['name'], v['version']))


class HtmlText(HTMLParser):
    def __init__(self):
        super().__init__()
        self.parts = []
    def handle_data(self, text):
        self.parts.append(text)
    def handle_starttag(self, tag, attrs):
        if tag in ('p', 'br', 'li', 'h1', 'h2', 'h3', 'pre'):
            self.parts.append('\n')


def header(text):
    """Retain the original leading source comments, including copyright notices."""
    match = re.match(r'\s*((?:(?://[^\n]*(?:\n|$))|(?:/\*[\s\S]*?\*/)|(?:#[^\n]*(?:\n|$))|\s)+)', text)
    if match and re.search(r'copyright|spdx|licen[sc]e|permission|redistribution', match[1], re.I):
        return match[1].strip() + '\n'
    return ''


def crate_evidence(row):
    name, version = row['package'], row['version']
    caches = list((Path.home() / '.cargo/registry/cache').glob(f'*/{name}-{version}.crate'))
    data = caches[0].read_bytes() if caches else fetch(row['source_archive'])
    require(digest(data) == row['sha256'], f'{name}: archive checksum mismatch')
    files = {}
    with tarfile.open(fileobj=io.BytesIO(data), mode='r:gz') as archive:
        for member in archive.getmembers():
            if member.isfile():
                relative = member.name.split('/', 1)[-1]
                files[relative] = archive.extractfile(member).read()
    package = files['Cargo.toml'].decode().split('[package]', 1)[1].split('\n[', 1)[0]
    license_match = re.search(r'^license = "([^"]+)"', package, re.M)
    require(license_match and license_match[1] == row['declared_license'],
            f'{name}: license metadata differs from reviewed inventory')
    require(row['available_license_option'] in ALLOWED, f'{name}: unreviewed license selection')
    retained = {p: value for p, value in files.items() if notice_name(p)}
    origins = {p: row['source_archive'] + '#' + p for p in retained}
    # Packages missing license files carry their precise upstream revision in the archive.
    metadata_only = name + '@' + version in json.loads(POLICY.read_text())['metadata_only_packages']
    if not retained and metadata_only:
        for path in ('Cargo.toml', 'Cargo.toml.orig', 'README.md'):
            if path in files:
                retained['upstream-declarations/' + path + '.txt'] = files[path]
                origins['upstream-declarations/' + path + '.txt'] = row['source_archive'] + '#' + path
        selected = row['available_license_option']
        if selected == 'MIT':
            terms = (ROOT / 'LICENSE').read_text().split('Permission is hereby granted', 1)[1]
            retained['MIT-TERMS.txt'] = ('MIT standard permission and disclaimer terms\n\nPermission is hereby granted' + terms).encode()
            origins['MIT-TERMS.txt'] = 'MIT standard terms; original upstream authors and license declarations retained separately'
        else:
            retained['Apache-2.0.txt'] = fetch('https://www.apache.org/licenses/LICENSE-2.0.txt')
            origins['Apache-2.0.txt'] = 'https://www.apache.org/licenses/LICENSE-2.0.txt'
        retained['EVIDENCE-NOTE.txt'] = ('Upstream declares the selected license but supplies no standalone copyright notice in the\nversioned package. Original authors and license declarations are retained.\nNo copyright holder or year has been inferred. The accompanying standard license terms are supplemental to the original\ndeclarations; no copyright line has been invented.\nThis documents the evidence available; it is not a legal certification.\n').encode()
        origins['EVIDENCE-NOTE.txt'] = 'Project evidence limitation documentation'
    if not retained:
        require('.cargo_vcs_info.json' in files, f'{name}: no licenses or upstream revision')
        vcs = json.loads(files['.cargo_vcs_info.json'])
        revision = vcs['git']['sha1']
        repository = row['repository'].removesuffix('.git').removeprefix('https://github.com/')
        require('/' in repository and not repository.startswith('http'), f'{name}: unsupported upstream')
        tree = json.loads(fetch(f'https://api.github.com/repos/{repository}/git/trees/{revision}?recursive=1'))
        require(not tree.get('truncated'), f'{name}: incomplete upstream tree')
        package_path = vcs.get('path_in_vcs', '').strip('/')
        parents = {'.', *[str(p) for p in Path(package_path or '.').parents], package_path or '.'}
        for entry in tree['tree']:
            path = entry['path']
            if entry['type'] == 'blob' and notice_name(path) and str(Path(path).parent) in parents:
                url = f'https://raw.githubusercontent.com/{repository}/{revision}/{path}'
                retained['upstream/' + path] = fetch(url)
                origins['upstream/' + path] = url
        require(retained, f'{name}: upstream license evidence missing at {revision}')
    # Metadata licenses alone omit notices attached to copied source material.
    for path, value in files.items():
        if path.endswith(('.rs', '.c', '.h')):
            text = header(value.decode(errors='replace'))
            if text and re.search(r'copyright', text, re.I):
                retained['source-headers/' + path + '.txt'] = text.encode()
                origins['source-headers/' + path + '.txt'] = row['source_archive'] + '#' + path
    return row, retained, origins


def build_context(args):
    build = args.idf_build_dir.resolve()
    project = json.loads((build / 'project_description.json').read_text())
    sdk = Path(project['idf_path']).resolve()
    require(sdk.is_dir(), f'SDK source tree missing: {sdk}; rebuild ESP-IDF')
    expected = json.loads(SDK_INVENTORY.read_text())
    require(run('git', '-C', sdk, 'rev-parse', 'HEAD') == expected['idf_git_tree'], 'Unsupported SDK revision')
    require(project['target'] == 'esp32', 'Only classic ESP32 is reviewed')
    for module in expected['submodules']:
        path = sdk / module['path']
        require(path.is_dir(), f'SDK submodule absent: {module["path"]}')
        require(run('git', '-C', path, 'rev-parse', 'HEAD') == module['revision'],
                f'Unsupported SDK submodule: {module["path"]}')
    compiler = Path(project['c_compiler']).resolve()
    toolchain = args.toolchain_dir.resolve()
    require(compiler.is_relative_to(toolchain), 'Toolchain directory does not contain the build compiler')
    maps = [build / 'libespidf.map', build / 'bootloader/bootloader.map']
    require(all(p.is_file() for p in maps), 'Application and bootloader linker maps are required')
    live_maps = [p.read_text().split('Linker script and memory map', 1)[-1].split('Cross Reference Table', 1)[0] for p in maps]
    archive_names = sorted(set(re.findall(r'([^\s()]+\.a)\(', '\n'.join(live_maps))))
    component_set = set()
    for archive in archive_names:
        match = re.search(r'(?:^|/)components/([^/]+)/', archive) or re.search(r'(?:^|/)esp-idf/([^/]+)/', archive)
        if match:
            component_set.add(match[1])
    components = sorted(component_set)
    # Map paths are not portable; classify the component/archive names instead.
    archive_ids = sorted({re.sub(r'^.*?/components/', 'components/', a) if '/components/' in a
                          else re.sub(r'^.*?/esp-idf/', 'esp-idf/', a) if '/esp-idf/' in a
                          else Path(a).name for a in archive_names})
    compile_files = set()
    notice_roots = set()
    for directory in (build, build / 'bootloader'):
        description = json.loads((directory / 'project_description.json').read_text())
        for name, component_path in zip(description['build_components'], description['build_component_paths']):
            path = Path(component_path).resolve()
            if name in components and path.is_relative_to(sdk):
                notice_roots.add(str(path.relative_to(sdk)))
        commands = directory / 'compile_commands.json'
        require(commands.is_file(), f'Missing compilation evidence: {commands}')
        for entry in json.loads(commands.read_text()):
            path = Path(entry['file']).resolve()
            output_match = re.search(r'\s-o\s+([^\s]+)', entry.get('command', ''))
            object_path = entry.get('output', '') or (output_match[1] if output_match else '')
            component_match = re.search(r'esp-idf/([^/]+)/', object_path)
            if path.is_relative_to(sdk) and component_match and component_match[1] in components:
                compile_files.add(str(path.relative_to(sdk)))
    # Ninja's dependency database gives the headers used by relevant component compilations.
    for directory in (build, build / 'bootloader'):
        current = False
        for line in run('ninja', '-C', directory, '-t', 'deps').splitlines():
            if line and not line[0].isspace():
                match = re.search(r'esp-idf/([^/]+)/', line)
                current = bool(match and match[1] in components)
            elif current:
                path = Path(line.strip())
                if path.is_absolute() and path.is_file() and path.resolve().is_relative_to(sdk):
                    compile_files.add(str(path.resolve().relative_to(sdk)))
    evidence = {'sdk_revision': expected['idf_git_tree'], 'idf_version': expected['idf_version'],
                'target': 'esp32', 'compiler_version': run(compiler, '--version'),
                'rust_version': rust_version(), 'archives': archive_ids,
                'compiled_sdk_sources': sorted(compile_files), 'sdk_components': components, 'sdk_notice_roots': sorted(notice_roots)}
    policy = json.loads(POLICY.read_text())
    for actual, reviewed in [('sdk_revision', 'reviewed_sdk_revision'), ('compiler_version', 'reviewed_compiler_version'), ('rust_version', 'reviewed_rust_version')]:
        require(evidence[actual] == policy[reviewed], f'Unreviewed build version: {actual}; review licensing-policy.json before refreshing')
    return build, sdk, toolchain, evidence


def update(args):
    require(lock_packages() == {(r['package'], r['version']) for r in rows()},
            'Lockfile differs from reviewed Rust inventory; review it before update')
    selected = target_packages()
    require(selected == json.loads(POLICY.read_text())['rust_packages'], 'ESP32 dependency graph changed; review licensing-policy.json')
    selection = {(p['name'], p['version']): p['coverage'] for p in selected}
    selected_rows = [r for r in rows() if (r['package'], r['version']) in selection]
    build, sdk, toolchain, evidence = build_context(args)
    with tempfile.TemporaryDirectory(prefix='.license-update-', dir=COLLECTION.parent) as temp:
        out = Path(temp) / 'collection'
        out.mkdir()
        manifest = {'format': 2, 'scope': 'ESP32 target dependencies, code-generation notices and linked SDK/runtime coverage; not a legal certification',
                    'inputs': {p: digest((ROOT / p).read_bytes()) for p in INPUTS},
                    'build': evidence, 'components': [], 'files': {}, 'build_inputs': {}, 'notice_parts': {}}
        parts = {}

        def put(relative, data, origin):
            require(not Path(relative).is_absolute() and '..' not in Path(relative).parts, f'Unsafe source path: {relative}')
            if not relative.startswith('sources/') and relative != 'PROJECT-LICENSE.txt':
                parts[relative] = data
                manifest['notice_parts'][relative] = {'sha256': digest(data), 'origin': origin}
                return
            path = out / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
            manifest['files'][relative] = {'sha256': digest(data), 'origin': origin}

        def retain_tree(source, prefix, origin, all_files=False, required=True):
            count = 0
            for path in sorted(source.rglob('*')):
                if path.is_file() and '.git' not in path.parts and (all_files or notice_name(path.name)):
                    data = path.read_bytes()
                    try:
                        data.decode('utf-8')
                    except UnicodeDecodeError:
                        continue
                    put(prefix + '/' + str(path.relative_to(source)), data, origin)
                    count += 1
            require(count or not required, f'No notice files in {source}')

        with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:
            for row, texts, origins in pool.map(crate_evidence, selected_rows):
                prefix = f'rust/{row["package"]}-{row["version"]}'
                paths = []
                for relative, value in sorted(texts.items()):
                    destination = prefix + '/' + relative
                    put(destination, value, origins[relative])
                    paths.append(destination)
                manifest['components'].append({'name': row['package'], 'version': row['version'],
                    'license': row['available_license_option'], 'coverage': selection[(row['package'], row['version'])],
                    'source': row['source_archive'], 'archive_sha256': row['sha256'], 'notices': paths})
        # Limit SDK notices to components observed in the application/bootloader maps.
        for relative in evidence['sdk_notice_roots']:
            retain_tree(sdk / relative, 'sdk/notices/' + relative,
                        f'ESP-IDF {evidence["sdk_revision"]}, including pinned submodules', required=False)
        # File-level notices take precedence over a repository-wide license summary.
        headers = []
        for relative in evidence['compiled_sdk_sources']:
            path = sdk / relative
            text = header(path.read_text(errors='replace'))
            if text:
                headers.append(f'\n===== {relative} =====\n{text}')
        require(headers, 'SDK file-level copyright evidence missing')
        put('sdk/SOURCE-NOTICES.txt', ''.join(headers).encode(), 'Original SDK source comment headers')
        for relative in ('LICENSE', 'COPYRIGHT'):
            if (sdk / relative).is_file():
                put('sdk/' + relative, (sdk / relative).read_bytes(), f'ESP-IDF {evidence["sdk_revision"]}/{relative}')
        # Pin every compiled SDK input, config and SDK/runtime archive used in linking.
        for relative in evidence['compiled_sdk_sources']:
            manifest['build_inputs']['sdk:' + relative] = digest((sdk / relative).read_bytes())
        for path in sorted(sdk.rglob('*.a')):
            if path.name in {Path(a).name for a in evidence['archives']}:
                manifest['build_inputs']['sdk:' + str(path.relative_to(sdk))] = digest(path.read_bytes())
        manifest['build_inputs']['build:config/sdkconfig.json'] = digest((build / 'config/sdkconfig.json').read_bytes())
        for path in sorted(toolchain.rglob('*.a')):
            if path.name in {'libgcc.a', 'libstdc++.a', 'libc.a', 'libm.a'}:
                manifest['build_inputs']['toolchain:' + str(path.relative_to(toolchain))] = digest(path.read_bytes())
        require(any(k.startswith('toolchain:') for k in manifest['build_inputs']), 'Runtime archives missing')
        for relative in ('gcc/COPYING3', 'gcc/COPYING.RUNTIME', 'newlib/COPYING.NEWLIB'):
            path = toolchain / 'share/licenses' / relative
            require(path.is_file(), f'Runtime notice missing: {relative}')
            put('runtime/gcc/' + relative, path.read_bytes(), 'Actual compiler runtime distribution')
        retain_tree(toolchain / 'share/licenses/gcc/libstdc++-v3', 'runtime/gcc/libstdc++',
                    'Actual libstdc++ runtime distribution', required=False)
        rust = rust_root()
        report = rust / 'share/doc/rust/COPYRIGHT-library.html'
        require(report.is_file(), 'Rust standard-library copyright report missing')
        parser = HtmlText()
        parser.feed(report.read_text())
        put('runtime/rust/COPYRIGHT-library.txt', ''.join(parser.parts).encode(),
            evidence['rust_version'] + '; plain-text export of supplied standard-library copyright report')
        library_lock = rust / 'lib/rustlib/src/rust/library/Cargo.lock'
        manifest['build_inputs']['rust:lib/rustlib/src/rust/library/Cargo.lock'] = digest(library_lock.read_bytes())
        # Include original project asset notices without modifying their terms.
        for path in sorted((ROOT / 'firmware/assets').rglob('*')):
            if path.is_file():
                manifest['inputs'][str(path.relative_to(ROOT))] = digest(path.read_bytes())
        for group in ('fonts', 'weather', 'ui', 'licenses'):
            source = ROOT / 'firmware/assets' / group
            for path in sorted(source.iterdir()):
                if path.is_file() and (path.suffix == '.txt' or path.name == 'sources.json'):
                    put('assets/' + group + '/' + path.name, path.read_bytes(), str(path.relative_to(ROOT)))
                    manifest['inputs'][str(path.relative_to(ROOT))] = digest(path.read_bytes())
        certdir = sdk / 'components/mbedtls/esp_crt_bundle'
        for name in ('cacrt_all.pem', 'gen_crt_bundle.py'):
            path = certdir / name
            require(path.is_file(), f'MPL certificate source missing: {name}')
            put('sources/certificates/' + name, path.read_bytes(), f'ESP-IDF {evidence["sdk_revision"]}/components/mbedtls/esp_crt_bundle/{name}')
            manifest['build_inputs']['sdk:' + str(path.relative_to(sdk))] = digest(path.read_bytes())
        put('sources/certificates/sdkconfig.json', (build / 'config/sdkconfig.json').read_bytes(), 'Actual build configuration')
        put('sources/certificates/MPL-2.0.txt', fetch('https://www.mozilla.org/media/MPL/2.0/index.txt'), 'https://www.mozilla.org/MPL/2.0/')
        certreadme = '''Certificate source material (MPL-2.0)\n\nThe bundled cacrt_all.pem is the exact input to ESP-IDF's certificate-bundle\nconversion. Its header preserves the Mozilla attribution, extraction date,\nupstream source reference and conversion provenance. gen_crt_bundle.py and\nsdkconfig.json describe the conversion and selection used by this firmware.\nModify the PEM input and rebuild the firmware to change its trust store.\nThese source materials remain available under MPL-2.0; the project's MIT\nlicense does not replace their terms. No additional restriction is imposed\non recipients' rights to this material.\n\nMozilla original certificate data: https://hg.mozilla.org/projects/nss/file/tip/lib/ckfw/builtins/certdata.txt\nPEM extraction tool and information: https://curl.se/docs/caextract.html\n'''
        put('sources/certificates/README.txt', certreadme.encode(), 'Project documentation')
        notice = ['# Third-party licenses and distribution notices\n',
            'The project code is MIT-licensed. Third-party software, fonts, icons and certificate data retain their own licenses.\n',
            'This package covers ESP32 normal dependencies, relevant macro providers, observed SDK components and compiler runtime libraries. It excludes build/test-only crates and compiler executables. Target dependencies may be optimized out; standard-library reports and notices within a relevant SDK component can conservatively cover additional files. The manifest records the scope. Checks validate evidence, not legal compliance.\n',
            'SDK original code: Apache-2.0; FreeRTOS: MIT; lwIP/wpa_supplicant/Newlib/TLSF and other components: their original notices. Mbed TLS is used under its Apache-2.0 option. Espressif Wi-Fi/PHY (and coexistence if enabled) remain Apache-2.0 with precompiled implementation. Original notices are consolidated below.\n',
            'Rust libraries: MIT/Apache as offered, plus their retained supplemental notices. GCC runtime: GPL with the GCC Runtime Library Exception, applicable to eligible compilation processes; no GCC sources are relicensed. Runtime notices are consolidated below.\n',
            'Montserrat subsets remain OFL-1.1; LVGL, Meteocons and Tabler retain their respective MIT notices; the TFT_eSPI adaptation retains MIT/BSD notices. Asset notices are consolidated below.\n',
            'Certificate data remains MPL-2.0. Its source input, converter and configuration are in sources/certificates/.\n',
            'Weather/location data: Open-Meteo and GeoNames, CC BY 4.0 attribution. The free Open-Meteo service is for non-commercial use; MIT does not grant commercial API access. https://open-meteo.com/en/terms\n',
            'Optional SDK examples/resources retain their own terms; this collection does not authorize their reuse. Freenove documentation/resources are not relicensed by this project.\n',
            '## Rust dependency coverage\n', '| Package | Version | Selected license | Notices |', '| --- | --- | --- | --- |']
        for item in manifest['components']:
            prefix = f'rust/{item["name"]}-{item["version"]}'
            notice.append(f'| {item["name"]} | {item["version"]} | {item["license"]} | {prefix} |')
        put('SUMMARY.txt', ('\n\n'.join(notice[:10]) + '\n\n' + '\n'.join(notice[10:]) + '\n').encode(), 'Project summary; original notices govern')
        put('PROJECT-LICENSE.txt', (ROOT / 'LICENSE').read_bytes(), 'LICENSE')
        # Consolidate original texts; deduplicate identical bodies, never their attribution.
        groups = {}
        for relative, value in sorted(parts.items()):
            groups.setdefault(digest(value), []).append(relative)
        document = bytearray(b'THIRD-PARTY NOTICES\nProject code: MIT. Included components retain their original licenses.\nEach evidence label is followed by its origin and original text.\n\n')
        for checksum, relatives in groups.items():
            labels = '\n'.join(relative + '\nSource: ' + manifest['notice_parts'][relative]['origin'] for relative in relatives)
            document.extend(('\n' + '=' * 72 + '\n' + labels + '\n' + '-' * 72 + '\n').encode())
            offset = len(document)
            body = parts[relatives[0]]
            document.extend(body)
            document.extend(b'\n')
            for relative in relatives:
                manifest['notice_parts'][relative].update(offset=offset, length=len(body))
        path = out / 'THIRD-PARTY-NOTICES.txt'
        path.write_bytes(document)
        manifest['files']['THIRD-PARTY-NOTICES.txt'] = {'sha256': digest(document), 'origin': 'Consolidated original notices; duplicate bodies share all evidence labels'}
        (out / 'manifest.json').write_text(json.dumps(manifest, indent=2, sort_keys=True) + '\n')
        previous = Path(temp) / 'previous'
        if COLLECTION.exists():
            COLLECTION.rename(previous)
        try:
            out.rename(COLLECTION)
        except OSError:
            if previous.exists():
                previous.rename(COLLECTION)
            raise
    print(f'Updated {COLLECTION}: {len(manifest["components"])} Rust packages and {len(manifest["files"])} evidence files')


def check():
    manifest = json.loads((COLLECTION / 'manifest.json').read_text())
    require(manifest.get('format') == 2, 'Unsupported evidence manifest format')
    for relative, expected in manifest['inputs'].items():
        path = ROOT / relative
        require(path.is_file() and digest(path.read_bytes()) == expected,
                f'Changed/missing input: {relative}; review changes and run update')
    packages = {(item['name'], item['version']) for item in manifest['components']}
    expected = {(p['name'], p['version']) for p in json.loads(POLICY.read_text())['rust_packages']}
    require(packages == expected and packages <= lock_packages() and lock_packages() == {(r['package'], r['version']) for r in rows()}, 'Dependency coverage mismatch')
    for item in manifest['components']:
        require(item['license'] in ALLOWED and item['notices'], f'Missing/unreviewed notices: {item["name"]}')
        require(all(p in manifest['notice_parts'] for p in item['notices']), f'Unrecorded notices: {item["name"]}')
    actual = {str(p.relative_to(COLLECTION)) for p in COLLECTION.rglob('*') if p.is_file()}
    require(actual == set(manifest['files']) | {'manifest.json'}, 'Evidence file list mismatch')
    for relative, item in manifest['files'].items():
        path = COLLECTION / relative
        require(not path.is_symlink() and path.is_relative_to(COLLECTION) and '..' not in Path(relative).parts,
                f'Unsafe evidence path: {relative}')
        require(digest(path.read_bytes()) == item['sha256'], f'Altered evidence: {relative}')
    for required in ('PROJECT-LICENSE.txt', 'THIRD-PARTY-NOTICES.txt',
                     'sources/certificates/cacrt_all.pem', 'sources/certificates/gen_crt_bundle.py',
                     'sources/certificates/MPL-2.0.txt', 'sources/certificates/sdkconfig.json',
                     'sources/certificates/README.txt'):
        require(required in manifest['files'], f'Required evidence missing: {required}')
    consolidated = (COLLECTION / 'THIRD-PARTY-NOTICES.txt').read_bytes()
    for name, part in manifest['notice_parts'].items():
        body = consolidated[part['offset']:part['offset'] + part['length']]
        require(digest(body) == part['sha256'], f'Altered consolidated notice: {name}')
    for name in ('assets/fonts/OFL.txt', 'assets/fonts/LVGL-MIT.txt', 'assets/fonts/NOTICE.txt',
                 'assets/weather/MIT.txt', 'assets/ui/MIT.txt', 'assets/licenses/TFT_eSPI.txt',
                 'sdk/LICENSE', 'runtime/rust/COPYRIGHT-library.txt', 'runtime/gcc/gcc/COPYING.RUNTIME'):
        require(name in manifest['notice_parts'], f'Required evidence missing: {name}')
    policy = json.loads(POLICY.read_text())
    for actual, reviewed in [('sdk_revision', 'reviewed_sdk_revision'), ('compiler_version', 'reviewed_compiler_version'), ('rust_version', 'reviewed_rust_version')]:
        require(manifest['build'][actual] == policy[reviewed], f'Unreviewed build version: {actual}')
    require(manifest['build']['idf_version'] == 'v5.5.5' and manifest['build']['target'] == 'esp32', 'Unsupported SDK/target')
    require(any(p.endswith('COPYING.RUNTIME') for p in manifest['notice_parts']), 'GCC exception missing')
    print('License evidence check passed (coverage/integrity, not legal certification)')
    return manifest


def package(args):
    manifest = check()
    build, sdk, toolchain, evidence = build_context(args)
    require(evidence == manifest['build'], 'Build/SDK/runtime coverage changed; review and run update')
    locations = {'sdk': sdk, 'build': build, 'toolchain': toolchain, 'rust': rust_root()}
    for key, expected in manifest['build_inputs'].items():
        kind, relative = key.split(':', 1)
        path = locations[kind] / relative
        require(path.is_file() and digest(path.read_bytes()) == expected, f'Build evidence changed: {key}')
    images = args.firmware_dir.resolve()
    for name in IMAGES:
        require((images / name).is_file() and (images / name).stat().st_size, f'Missing image: {name}')
    for name, relative in [('bootloader.bin', 'bootloader/bootloader.bin'),
                           ('partition-table.bin', 'partition_table/partition-table.bin')]:
        require((images / name).read_bytes() == (build / relative).read_bytes(), f'Mismatched build image: {name}')
    require(re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9._-]*', args.version), 'Unsafe version string')
    with (ROOT / 'firmware/partitions.csv').open() as source:
        app = next(row for row in csv.reader(line for line in source if not line.lstrip().startswith('#'))
                   if row and row[0].strip() == 'app0')
    require(int(app[3].strip(), 0) == 0x10000 and (images / IMAGES[2]).stat().st_size <= int(app[4].strip(), 0), 'Unsupported partition layout/image size')
    config = json.loads((build / 'config/sdkconfig.json').read_text())
    require(config.get('ESPTOOLPY_FLASHSIZE') == '4MB', 'Unsupported flash size')
    output = args.output_dir.resolve()
    stem = 'weather-forecast-' + args.version
    require(not any((output / (stem + suffix)).exists() for suffix in ('.zip', '.tar.gz')), 'Output archive already exists')
    with tempfile.TemporaryDirectory(prefix='weather-release-') as temp:
        base = Path(temp)
        regenerated = base / 'verified-app.bin'
        elf = images / 'weather-forecast-firmware'
        require(elf.is_file(), f'Missing release ELF: {elf}')
        subprocess.run([args.esptool_python, '-m', 'esptool', '--chip', 'esp32', 'elf2image',
                        '--flash_mode', 'dio', '--flash_freq', '40m', '--flash_size', '4MB',
                        '--output', str(regenerated), str(elf)], check=True, stdout=subprocess.DEVNULL)
        require(regenerated.read_bytes() == (images / IMAGES[2]).read_bytes(), 'Application image does not match release ELF')
        require(b'libnet80211.a' in (build / 'libespidf.map').read_bytes(), 'Incomplete application linkage evidence')
        dest = base / stem
        dest.mkdir()
        for name in IMAGES:
            shutil.copyfile(images / name, dest / name)
        shutil.copytree(COLLECTION, dest / 'licenses')
        shutil.copyfile(ROOT / 'LICENSE', dest / 'LICENSE')
        text = f'''ESP32 weather forecast screen {args.version}\n\nProject code: MIT. Included components retain their original licenses.\nRead licenses/THIRD-PARTY-NOTICES.txt and the original texts before redistribution.\nWhen distributing extracted binaries, provide these accompanying materials too.\nCertificate source material: licenses/sources/certificates/.\nThe declaration is copied as a reference; its relative links refer to the\nproject source repository. The complete release license texts are in licenses/.\n\nClassic ESP32 / 4 MiB flash only. Install esptool on your host, connect the\nboard, and replace /dev/ttyUSB0 with its serial port:\n\npython3 -m esptool --chip esp32 --port /dev/ttyUSB0 --baud 460800 write_flash --flash_mode dio --flash_freq 40m --flash_size 4MB 0x1000 bootloader.bin 0x8000 partition-table.bin 0x10000 weather-forecast-firmware.bin\n\nThese offsets preserve the existing settings/filesystem partitions. No flash\nerase, eFuse programming, firmware backup or personal credentials are included.\n\nWeather/location data: Open-Meteo and GeoNames; CC BY 4.0. Free API use is\nnon-commercial: https://open-meteo.com/en/terms\n\nChecksums verify integrity; they are not signatures or legal certification.\n'''
        (dest / 'README.txt').write_text(text)
        provenance = {'release': args.version, 'project_commit': run('git', '-C', ROOT, 'rev-parse', 'HEAD'),
                      'working_tree_dirty': bool(run('git', '-C', ROOT, 'status', '--porcelain')),
                      'elf_sha256': digest(elf.read_bytes()), 'evidence': evidence}
        (dest / 'BUILD-PROVENANCE.json').write_text(json.dumps(provenance, indent=2, sort_keys=True) + '\n')
        sums = ''.join(f'{digest(p.read_bytes())}  {p.relative_to(dest)}\n' for p in sorted(dest.rglob('*')) if p.is_file())
        (dest / 'SHA256SUMS').write_text(sums)
        output.mkdir(parents=True, exist_ok=True)
        with zipfile.ZipFile(output / (stem + '.zip'), 'w', zipfile.ZIP_DEFLATED) as archive:
            for path in sorted(dest.rglob('*')):
                if path.is_file():
                    archive.write(path, str(path.relative_to(base)))
        with tarfile.open(output / (stem + '.tar.gz'), 'w:gz') as archive:
            archive.add(dest, arcname=stem)
    print(f'Created {output / (stem + ".zip")} and .tar.gz; nothing published')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    for command in ('update', 'check', 'package'):
        sub = commands.add_parser(command)
        if command in ('update', 'package'):
            sub.add_argument('--idf-build-dir', type=Path, required=True)
            sub.add_argument('--toolchain-dir', type=Path, required=True)
        if command == 'package':
            sub.add_argument('--firmware-dir', type=Path, required=True)
            sub.add_argument('--version', required=True)
            sub.add_argument('--output-dir', type=Path, required=True)
            sub.add_argument('--esptool-python', default=sys.executable)
    args = parser.parse_args()
    try:
        {'update': update, 'check': lambda _: check(), 'package': package}[args.command](args)
    except (ValueError, OSError, KeyError, subprocess.CalledProcessError) as error:
        parser.exit(1, f'License release error: {error}\n')


if __name__ == '__main__':
    main()
