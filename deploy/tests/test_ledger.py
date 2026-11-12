"""Transport contracts use fake crypto; the Docker smoke uses actual age and Rust/PG."""
import base64
import copy
import hashlib
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch
import uuid

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import ledger

NOW = 2_000_000_000


def fake_encrypt(value, path, *_):
    data = ledger.encode(value)
    path.write_bytes(hashlib.sha256(data).digest() + base64.b85encode(data))


def fake_decrypt(path, *_):
    raw = path.read_bytes()
    data = base64.b85decode(raw[32:])
    ledger.require(raw[:32] == hashlib.sha256(data).digest())
    return ledger.decode(data)


class LedgerContracts(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name) / 'ledger'
        self.root.mkdir()
        self.key = Path(self.temp.name) / 'key'
        self.key.write_text('synthetic key')
        self.now = NOW
        self.calls = []
        self.audit_matches = True
        self.snapshot = {'version': 1, 'source_id': str(uuid.uuid4()), 'captured_at': NOW, 'entries': [
            {'kind': 'account', 'id': str(uuid.uuid4()), 'deleted_at': NOW-2419199, 'expires_at': NOW+1},
            {'kind': 'identity', 'id': str(uuid.uuid4()), 'deleted_at': NOW-10, 'expires_at': NOW-10+2419200}]}
        self.patches = [patch.object(ledger, 'native', self.native), patch.object(ledger, 'encrypt', fake_encrypt), patch.object(ledger, 'decrypt', fake_decrypt)]
        for p in self.patches:
            p.start()

    def tearDown(self):
        for p in reversed(self.patches):
            p.stop()
        self.temp.cleanup()

    def native(self, args, data, *_):
        self.calls.append(args[0])
        if args == ['capture']:
            return copy.deepcopy(self.snapshot)
        value = ledger.decode(data)
        if args == ['inspect-snapshot']:
            # Only a port fixture; Rust model rules are tested by the real binary.
            return {'source_id': value['source_id'], 'captured_at': value['captured_at'], 'entries': len(value['entries'])}
        if args == ['compare-snapshot']:
            ledger.require(self.audit_matches)
            return {'captured_at': value['captured_at'], 'observed_at': self.now, 'entries': len(value['entries'])}
        self.assertEqual(args[0], 'apply-snapshot')
        return {'state': 'quarantined', 'applied': len(value['entries'])}

    def store(self):
        return ledger.LedgerStore(self.root, lambda: self.now)

    def capture(self):
        with self.store() as store:
            store.capture(self.key, {})
        return ledger.read_json(self.root / 'current.json')

    def files(self):
        pointer = ledger.read_json(self.root / 'current.json')
        folder = self.root / pointer['name']
        return folder, ledger.read_json(folder / 'manifest.json')

    def test_public_metadata_has_no_personal_ids_and_success_is_quarantined(self):
        self.capture()
        folder, manifest = self.files()
        self.assertEqual(len(manifest['parts']), 2)
        public = (folder / 'manifest.json').read_text() + (self.root / 'current.json').read_text() + ' '.join(p.name for p in folder.iterdir())
        for e in self.snapshot['entries']:
            self.assertNotIn(e['id'], public)
        with self.store() as store:
            self.assertEqual(store.apply(self.key, str(uuid.uuid4()), {}), {'state': 'quarantined', 'applied': 2})
        self.assertEqual(self.calls[-2:], ['inspect-snapshot', 'apply-snapshot'])

    def test_exact_original_expiry_removes_only_the_expired_ciphertext(self):
        pointer = self.capture()
        folder, manifest = self.files()
        old = next(p for p in manifest['parts'] if p['expires_at'] == NOW+1)
        self.now += 1
        with self.store() as store:
            self.assertEqual(store.verify(self.key), {'captured_at': NOW, 'entries': 1})
        self.assertFalse((folder / old['name']).exists())
        self.assertEqual(ledger.read_json(self.root / 'current.json'), pointer)
        self.assertEqual(len(list(folder.glob('part-*.age'))), 1)

    def test_record_expiring_during_index_authentication_is_also_physically_pruned(self):
        self.capture()
        folder, manifest = self.files()
        expiring = folder / manifest['parts'][0]['name']
        def advances_clock(path, *args):
            value = fake_decrypt(path, *args)
            if path.name == 'index.age':
                self.now = NOW+1
            return value
        with patch.object(ledger, 'decrypt', advances_clock), self.store() as store:
            self.assertEqual(store.verify(self.key), {'captured_at': NOW, 'entries': 1})
        self.assertFalse(expiring.exists())

    def test_repeated_capture_never_extends_original_entry_expiry(self):
        self.capture()
        self.now += 1
        self.snapshot['captured_at'] = self.now
        self.capture()
        _, manifest = self.files()
        self.assertEqual([p['expires_at'] for p in manifest['parts']], [NOW-10+2419200])
        self.assertEqual(len(list(self.root.glob('ledger-*'))), 1)

    def test_all_authentication_finishes_before_any_target_apply_call(self):
        self.capture()
        folder, manifest = self.files()
        last = folder / manifest['parts'][-1]['name']
        def fail_last(path, *args):
            if path == last:
                raise ledger.BackupError()
            return fake_decrypt(path, *args)
        self.calls.clear()
        with patch.object(ledger, 'decrypt', fail_last), self.store() as store:
            with self.assertRaises(ledger.BackupError):
                store.apply(self.key, str(uuid.uuid4()), {})
        self.assertNotIn('apply-snapshot', self.calls)

    def test_audit_keeps_the_original_capture_and_returns_only_a_point_in_time_match(self):
        pointer = self.capture()
        self.now += 301
        with self.store() as store:
            self.assertEqual(store.audit(self.key, {}), {'captured_at': NOW, 'observed_at': NOW+301, 'entries': 1})
        self.assertEqual(self.calls[-2:], ['inspect-snapshot', 'compare-snapshot'])
        self.assertNotIn('apply-snapshot', self.calls)
        self.assertEqual(ledger.read_json(self.root / 'current.json'), pointer)

    def test_authenticated_but_stale_audit_does_not_rewrite_or_apply_the_checkpoint(self):
        pointer = self.capture()
        self.audit_matches = False
        with self.store() as store:
            self.assertEqual(store.verify(self.key)['entries'], 2)
            with self.assertRaises(ledger.BackupError):
                store.audit(self.key, {})
        self.assertNotIn('apply-snapshot', self.calls)
        self.assertEqual(ledger.read_json(self.root / 'current.json'), pointer)

    def test_last_authentication_failure_prevents_even_reading_the_source_db(self):
        self.capture()
        folder, manifest = self.files()
        last = folder / manifest['parts'][-1]['name']
        def fail_last(path, *args):
            if path == last:
                raise ledger.BackupError()
            return fake_decrypt(path, *args)
        self.calls.clear()
        with patch.object(ledger, 'decrypt', fail_last), self.store() as store:
            with self.assertRaises(ledger.BackupError):
                store.audit(self.key, {})
        self.assertNotIn('compare-snapshot', self.calls)
        self.assertNotIn('apply-snapshot', self.calls)

    def test_missing_or_corrupt_ciphertext_never_reaches_apply(self):
        for damage in ['missing', 'corrupt']:
            with self.subTest(damage=damage):
                self.capture()
                folder, manifest = self.files()
                path = folder / manifest['parts'][-1]['name']
                original = path.read_bytes()
                path.unlink()
                if damage == 'corrupt':
                    path.write_bytes(original[:-1])
                self.calls.clear()
                with self.store() as store:
                    with self.assertRaises(ledger.BackupError):
                        store.apply(self.key, str(uuid.uuid4()), {})
                self.assertNotIn('apply-snapshot', self.calls)
                path.write_bytes(original)

    def test_encryption_failure_before_pointer_commit_preserves_previous_current(self):
        pointer = self.capture()
        with patch.object(ledger, 'encrypt', side_effect=ledger.BackupError()), self.store() as store:
            with self.assertRaises(ledger.BackupError):
                store.capture(self.key, {})
        self.assertEqual(ledger.read_json(self.root / 'current.json'), pointer)
        self.assertEqual(len(list(self.root.glob('ledger-*'))), 1)
        self.assertFalse(list(self.root.glob('pending-*')))

    def test_post_commit_fsync_failure_never_deletes_the_new_current(self):
        self.capture()
        original = ledger.Store.sync
        def fail_after_commit(path):
            pointer = ledger.read_json(self.root / 'current.json')
            if path == self.root and pointer['name'] != first['name']:
                raise OSError('synthetic post-commit failure')
            original(path)
        first = ledger.read_json(self.root / 'current.json')
        with patch.object(ledger.Store, 'sync', side_effect=fail_after_commit), self.store() as store:
            with self.assertRaises(OSError):
                store.capture(self.key, {})
        pointer = ledger.read_json(self.root / 'current.json')
        self.assertNotEqual(pointer['name'], first['name'])
        self.assertTrue((self.root / pointer['name'] / 'index.age').is_file())
        with self.store() as store:
            self.assertEqual(store.verify(self.key)['entries'], 2)

    def test_source_mismatch_older_capture_or_reversed_clock_is_rejected(self):
        pointer = self.capture()
        for field, value in [('source_id', str(uuid.uuid4())), ('captured_at', NOW-1)]:
            before = self.snapshot[field]
            self.snapshot[field] = value
            with self.store() as store:
                with self.assertRaises(ledger.BackupError):
                    store.capture(self.key, {})
            self.snapshot[field] = before
            self.assertEqual(ledger.read_json(self.root / 'current.json'), pointer)
        self.now -= 1
        with self.store() as store:
            with self.assertRaises(ledger.BackupError):
                store.prune()

    def test_expired_generation_keeps_only_nonpersonal_source_binding(self):
        pointer = self.capture()
        self.now += ledger.MAX_AGE+1
        with self.store() as store:
            store.prune()
            with self.assertRaises(ledger.BackupError):
                store.verify(self.key)
        pointer['name'] = None
        self.assertEqual(ledger.read_json(self.root / 'current.json'), pointer)
        self.assertFalse(list(self.root.glob('ledger-*')))

    def test_known_partial_pending_is_removed_but_unexpected_files_are_preserved(self):
        self.capture()
        folder = self.root / f'pending-{NOW}-{uuid.uuid4().hex}'
        folder.mkdir()
        (folder / 'manifest.json').write_text('interrupted partial JSON')
        (folder / f'part-{NOW+1}-{uuid.uuid4().hex}.age').write_bytes(b'partial')
        with self.store() as store:
            store.prune()
        self.assertFalse(folder.exists())
        foreign = self.root / 'user.txt'
        foreign.write_text('preserve')
        with self.store() as store:
            with self.assertRaises(ledger.BackupError):
                store.prune()
        self.assertEqual(foreign.read_text(), 'preserve')

    def test_store_links_and_file_links_are_never_followed_or_deleted(self):
        self.capture()
        folder, manifest = self.files()
        path = folder / manifest['parts'][0]['name']
        path.unlink()
        path.symlink_to(self.key)
        with self.store() as store:
            with self.assertRaises(ledger.BackupError):
                store.prune()
        self.assertEqual(self.key.read_text(), 'synthetic key')
        alias = self.root.parent / 'alias'
        alias.symlink_to(self.root, target_is_directory=True)
        with self.assertRaises(ledger.BackupError):
            with ledger.LedgerStore(alias):
                pass

    def test_duplicate_json_fields_and_concurrent_writer_are_rejected(self):
        with self.assertRaises(ledger.BackupError):
            ledger.decode(b'{"version":1,"version":1}')
        with self.store():
            with self.assertRaises(ledger.BackupError):
                with self.store():
                    pass


if __name__ == '__main__':
    unittest.main()
