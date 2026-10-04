"""Regression tests for release evidence failures; no network or hardware."""
import contextlib
import importlib.util
import io
import json
from pathlib import Path
import shutil
import tempfile
from types import SimpleNamespace
import unittest
from unittest import mock

MODULE_PATH = Path(__file__).resolve().parents[1] / 'license_release.py'
spec = importlib.util.spec_from_file_location('license_release', MODULE_PATH)
release = importlib.util.module_from_spec(spec)
spec.loader.exec_module(release)


class EvidenceTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.collection = self.root / 'docs/licensing'
        shutil.copytree(release.COLLECTION, self.collection)
        self.manifest = json.loads((self.collection / 'manifest.json').read_text())
        for relative in set(self.manifest['inputs']) | set(self.manifest['application']['inputs']):
            destination = self.root / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(release.ROOT / relative, destination)
        for name, value in [('ROOT', self.root), ('COLLECTION', self.collection),
                            ('INVENTORY', self.root / 'docs/third-party-rust-2026-10-04.csv'),
                            ('POLICY', self.root / 'docs/licensing-policy.json')]:
            patcher = mock.patch.object(release, name, value)
            patcher.start()
            self.addCleanup(patcher.stop)

    def check(self):
        with contextlib.redirect_stdout(io.StringIO()):
            return release.check()

    def save_manifest(self):
        (self.collection / 'manifest.json').write_text(json.dumps(self.manifest))

    def test_complete_collection_checks_offline_without_rewriting(self):
        before = (self.collection / 'manifest.json').read_bytes()
        with mock.patch.object(release, 'fetch', side_effect=AssertionError('network called')):
            self.check()
        self.assertEqual(before, (self.collection / 'manifest.json').read_bytes())

    def test_missing_elf_hash_fails_offline(self):
        self.manifest['application'].pop('elf_sha256')
        self.save_manifest()
        with self.assertRaisesRegex(ValueError, 'Missing/invalid application ELF hash'):
            self.check()

    def test_changed_application_input_fails(self):
        (self.root / 'firmware/src/main.rs').write_text('changed source')
        with self.assertRaisesRegex(ValueError, 'Application inputs changed'):
            self.check()

    def test_changed_dependency_fails(self):
        path = self.root / 'firmware/Cargo.lock'
        path.write_text(path.read_text().replace('version = "0.1.0"', 'version = "0.1.1"', 1))
        with self.assertRaisesRegex(ValueError, 'Application inputs changed'):
            self.check()

    def test_missing_original_notice_fails(self):
        (self.collection / 'THIRD-PARTY-NOTICES.txt').unlink()
        with self.assertRaisesRegex(ValueError, 'Evidence file list mismatch'):
            self.check()

    def test_modified_copyright_notice_fails(self):
        path = self.collection / 'THIRD-PARTY-NOTICES.txt'
        path.write_bytes(path.read_bytes().replace(b'Copyright', b'Removed copyright', 1))
        with self.assertRaisesRegex(ValueError, 'Altered evidence'):
            self.check()

    def test_missing_certificate_source_fails_even_if_removed_from_manifest(self):
        key = 'sources/certificates/cacrt_all.pem'
        (self.collection / key).unlink()
        del self.manifest['files'][key]
        self.save_manifest()
        with self.assertRaisesRegex(ValueError, 'Required evidence missing'):
            self.check()

    def test_incomplete_dependency_coverage_fails(self):
        self.manifest['components'].pop()
        self.save_manifest()
        with self.assertRaisesRegex(ValueError, 'Dependency coverage mismatch'):
            self.check()

    def test_unreviewed_license_selection_fails(self):
        self.manifest['components'][0]['license'] = 'LicenseRef-Unreviewed'
        self.save_manifest()
        with self.assertRaisesRegex(ValueError, 'Missing/unreviewed notices'):
            self.check()

    def test_unsupported_sdk_fails(self):
        self.manifest['build']['idf_version'] = 'v99.0.0'
        self.save_manifest()
        with self.assertRaisesRegex(ValueError, 'Unsupported SDK/target'):
            self.check()

    def test_unreviewed_runtime_fails(self):
        self.manifest['build']['rust_version'] = 'unreviewed compiler'
        self.save_manifest()
        with self.assertRaisesRegex(ValueError, 'Unreviewed build version'):
            self.check()

    def test_changed_partition_layout_fails(self):
        path = self.root / 'firmware/partitions.csv'
        path.write_text(path.read_text().replace('0x10000', '0x20000'))
        with self.assertRaisesRegex(ValueError, 'Application inputs changed'):
            self.check()

    def test_symlinked_notice_fails(self):
        path = self.collection / 'THIRD-PARTY-NOTICES.txt'
        data = path.read_bytes()
        path.unlink()
        external = self.root / 'external.txt'
        external.write_bytes(data)
        path.symlink_to(external)
        with self.assertRaisesRegex(ValueError, 'Unsafe evidence path'):
            self.check()


class PackagingFailureTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.build = self.root / 'build'
        self.images = self.root / 'images'
        for p in (self.build / 'config', self.build / 'bootloader',
                  self.build / 'partition_table', self.images, self.root / 'firmware'):
            p.mkdir(parents=True, exist_ok=True)
        cfg = self.build / 'config/sdkconfig.json'
        cfg.write_text(json.dumps({'ESPTOOLPY_FLASHSIZE': '4MB'}))
        self.manifest = {'build': {'runtime': 'reviewed'},
                         'build_inputs': {'build:config/sdkconfig.json': release.digest(cfg.read_bytes())}}
        for name, value in [('bootloader.bin', b'bootloader'),
                            ('partition-table.bin', b'partitions'),
                            ('weather-forecast-firmware.bin', b'actual application')]:
            (self.images / name).write_bytes(value)
        (self.build / 'bootloader/bootloader.bin').write_bytes(b'bootloader')
        (self.build / 'partition_table/partition-table.bin').write_bytes(b'partitions')
        (self.images / 'weather-forecast-firmware').write_bytes(b'release ELF')
        (self.root / 'firmware/partitions.csv').write_text('app0,app,ota_0,0x10000,0x300000,\n')
        self.manifest['application'] = {'elf_sha256': release.digest(b'release ELF'), 'inputs': {}}
        self.args = SimpleNamespace(idf_build_dir=self.build, toolchain_dir=self.root,
            firmware_dir=self.images, version='1.0.0', output_dir=self.root / 'output',
            esptool_python='python3')
        for target, value in [('ROOT', self.root), ('check', lambda: self.manifest),
                              ('rust_root', lambda: self.root), ('application_inputs', lambda: {}),
                              ('build_context', lambda _: (self.build, self.root, self.root, {'runtime': 'reviewed'}))]:
            patcher = mock.patch.object(release, target, value)
            patcher.start()
            self.addCleanup(patcher.stop)

    def assert_rejected(self, reason):
        with self.assertRaisesRegex(ValueError, reason):
            release.package(self.args)
        self.assertFalse(self.args.output_dir.exists(), 'Created output despite failed validation')

    def test_different_elf_with_matching_binary_is_rejected_before_regeneration(self):
        (self.images / 'weather-forecast-firmware').write_bytes(b'old ELF')
        (self.images / 'weather-forecast-firmware.bin').write_bytes(b'old matching binary')
        with mock.patch.object(release.subprocess, 'run', side_effect=AssertionError('regeneration called')):
            self.assert_rejected('Application ELF or inputs differ')

    def test_missing_application_hash_is_rejected(self):
        self.manifest['application'].pop('elf_sha256')
        self.assert_rejected('Application ELF or inputs differ')

    def test_changed_application_source_is_rejected(self):
        with mock.patch.object(release, 'application_inputs', return_value={'firmware/src/main.rs': 'changed'}):
            self.assert_rejected('Application ELF or inputs differ')

    def test_recorded_elf_packages_successfully(self):
        collection = self.root / 'licenses'
        collection.mkdir()
        (collection / 'notice.txt').write_text('original notices')
        (self.root / 'LICENSE').write_text('MIT')
        (self.build / 'libespidf.map').write_bytes(b'libnet80211.a')
        def regenerate(command, **kwargs):
            Path(command[command.index('--output') + 1]).write_bytes(b'actual application')
        with mock.patch.object(release, 'COLLECTION', collection), mock.patch.object(release, 'run', return_value='commit'), mock.patch.object(release.subprocess, 'run', side_effect=regenerate):
            release.package(self.args)
        self.assertEqual(len(list(self.args.output_dir.iterdir())), 2)

    def test_changed_runtime_is_rejected(self):
        self.manifest['build']['runtime'] = 'different runtime'
        self.assert_rejected('Build/SDK/runtime coverage changed')

    def test_changed_sdk_configuration_is_rejected(self):
        (self.build / 'config/sdkconfig.json').write_text('{}')
        self.assert_rejected('Build evidence changed')

    def test_missing_image_is_rejected(self):
        (self.images / 'weather-forecast-firmware.bin').unlink()
        self.assert_rejected('Missing image')

    def test_mismatched_bootloader_is_rejected(self):
        (self.images / 'bootloader.bin').write_bytes(b'other bootloader')
        self.assert_rejected('Mismatched build image')

    def test_unsafe_version_is_rejected(self):
        self.args.version = '../outside'
        self.assert_rejected('Unsafe version')

    def test_intermediate_sdk_application_is_rejected(self):
        (self.images / 'weather-forecast-firmware.bin').write_bytes(b'intermediate SDK application')
        def regenerate(command, **kwargs):
            Path(command[command.index('--output') + 1]).write_bytes(b'actual application')
        with mock.patch.object(release.subprocess, 'run', side_effect=regenerate):
            self.assert_rejected('Application image does not match release ELF')


if __name__ == '__main__':
    unittest.main()
