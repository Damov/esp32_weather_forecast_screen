"""Release trigger and archive verification regression tests."""
import hashlib
import importlib.util
import io
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import unittest
import zipfile

SCRIPT = Path(__file__).resolve().parents[1] / 'ci_release.py'
spec = importlib.util.spec_from_file_location('ci_release', SCRIPT)
ci = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ci)


class VersionTests(unittest.TestCase):
    def test_stable_and_prerelease_tags(self):
        for tag in ('v0.1.0', 'v12.34.56', 'v1.0.0-rc.1'):
            self.assertEqual(ci.release_version('push', tag, 'abcdef', '5'), tag[1:])

    def test_invalid_tags_and_events(self):
        for tag in ('v1', 'v01.2.3', 'v1.2.3-01', 'v1.2.3+meta', 'v1.2.3;echo secret', 'main'):
            with self.assertRaises(ValueError):
                ci.release_version('push', tag, 'abcdef', '5')
        with self.assertRaises(ValueError):
            ci.release_version('pull_request', 'main', 'abcdef', '5')

    def test_manual_build_name(self):
        self.assertEqual(ci.release_version('workflow_dispatch', 'dev', 'a' * 40, '123'), 'test-aaaaaaaaaaaa-123')


class ArchiveTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.output = Path(self.temp.name)
        self.version = '1.0.0'
        self.stem = 'weather-forecast-' + self.version
        self.payload = {'README.txt': b'Instructions', 'firmware.bin': b'Firmware'}

    def write_archives(self, tamper=False, omit_checksum=False):
        sums = ''.join(f'{hashlib.sha256(data).hexdigest()}  {name}\n' for name, data in self.payload.items() if not (omit_checksum and name == 'firmware.bin'))
        payload = {**self.payload, 'SHA256SUMS': sums.encode()}
        with zipfile.ZipFile(self.output / (self.stem + '.zip'), 'w') as archive:
            for name, data in payload.items():
                archive.writestr(self.stem + '/' + name, data)
        with tarfile.open(self.output / (self.stem + '.tar.gz'), 'w:gz') as archive:
            for name, data in payload.items():
                if tamper and name == 'firmware.bin':
                    data = b'other firmware'
                info = tarfile.TarInfo(self.stem + '/' + name)
                info.size = len(data)
                archive.addfile(info, io.BytesIO(data))

    def test_matching_archives_and_checksums(self):
        self.write_archives()
        ci.verify_archives(self.output, self.version)
        self.assertEqual(len((self.output / 'SHA256SUMS').read_text().splitlines()), 2)

    def test_different_payloads_fail(self):
        self.write_archives(tamper=True)
        with self.assertRaisesRegex(ValueError, 'payloads differ'):
            ci.verify_archives(self.output, self.version)

    def test_missing_payload_checksum_fails(self):
        self.write_archives(omit_checksum=True)
        with self.assertRaisesRegex(ValueError, 'Incomplete payload checksums'):
            ci.verify_archives(self.output, self.version)

    def test_corrupt_payload_checksum_fails(self):
        digest = ci.hashlib.sha256
        self.write_archives()
        self.payload['firmware.bin'] = b'changed'
        # Rebuild both archives with a deliberately stale checksum.
        from unittest import mock
        with mock.patch.object(ci.hashlib, 'sha256', side_effect=lambda data: digest(b'Firmware') if data == b'changed' else digest(data)):
            self.write_archives()
        with self.assertRaisesRegex(ValueError, 'Invalid payload checksum'):
            ci.verify_archives(self.output, self.version)

    def test_check_custom_collection_is_offline_and_does_not_rewrite(self):
        import shutil
        source = ci.ROOT / 'docs/licensing'
        collection = self.output / 'evidence'
        shutil.copytree(source, collection)
        before = {str(p.relative_to(collection)): p.read_bytes() for p in collection.rglob('*') if p.is_file()}
        subprocess.run([sys.executable, str(ci.ROOT / 'tools/license_release.py'), 'check', '--collection-dir', str(collection)], check=True, stdout=subprocess.DEVNULL)
        after = {str(p.relative_to(collection)): p.read_bytes() for p in collection.rglob('*') if p.is_file()}
        self.assertEqual(before, after)


if __name__ == '__main__':
    unittest.main()
