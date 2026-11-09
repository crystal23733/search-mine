"""Behavior contracts for backup publication, validation and bounded cleanup."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('backup', Path(__file__).resolve().parents[1] / 'backup.py')
backup = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(backup)


class BackupContracts(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name) / 'store'
        self.root.mkdir()
        self.now = 2_000_000_000
        self.store = backup.Store(self.root, self.now)

    def tearDown(self):
        self.temp.cleanup()

    def archive(self, age=0, pending=False):
        start = self.now - age
        name = f"{'pending' if pending else 'backup'}-{start}-{'a' * 32}"
        folder = self.root / name
        folder.mkdir()
        value = {'version': 1, 'started_at': start}
        if not pending:
            body = b'encrypted fixture'
            (folder / 'database.dump.age').write_bytes(body)
            value.update(size=len(body), sha256=hashlib.sha256(body).hexdigest())
        (folder / 'manifest.json').write_text(json.dumps(value))
        return name, folder

    def test_unchanged_valid_archive_is_accepted(self):
        name, _ = self.archive()
        self.store.validate(name)

    def test_tampering_is_rejected(self):
        name, folder = self.archive()
        (folder / 'database.dump.age').write_bytes(b'changed')
        with self.assertRaises(backup.BackupError):
            self.store.validate(name)

    def test_exact_expiry_is_rejected_and_pruned(self):
        name, folder = self.archive(backup.MAX_AGE)
        with self.assertRaises(backup.BackupError):
            self.store.validate(name)
        self.assertEqual(self.store.prune(), 1)
        self.assertFalse(folder.exists())

    def test_completed_metadata_in_interrupted_pending_is_pruned(self):
        name, folder = self.archive(backup.MAX_AGE)
        pending = folder.with_name(name.replace('backup-', 'pending-', 1))
        folder.rename(pending)
        self.assertEqual(self.store.prune(), 1)
        self.assertFalse(pending.exists())

    def test_all_candidates_are_checked_before_any_deletion(self):
        _, folder = self.archive(backup.MAX_AGE)
        invalid = self.root / 'pending-invalid'
        invalid.mkdir()
        with self.assertRaises(backup.BackupError):
            self.store.prune()
        self.assertTrue(folder.exists())

    def test_before_boundary_and_unrelated_files_are_retained(self):
        _, folder = self.archive(backup.MAX_AGE - 1)
        unrelated = self.root / 'user.txt'
        unrelated.write_text('keep')
        self.assertEqual(self.store.prune(), 0)
        self.assertTrue(folder.exists() and unrelated.exists())

    def test_mtime_does_not_extend_pending_retention(self):
        _, folder = self.archive(backup.MAX_AGE, pending=True)
        self.assertEqual(self.store.prune(), 1)
        self.assertFalse(folder.exists())

    def test_future_archive_fails_without_deletion(self):
        name, folder = self.archive(-1)
        with self.assertRaises(backup.BackupError):
            self.store.validate(name)
        with self.assertRaises(backup.BackupError):
            self.store.prune()
        self.assertTrue(folder.exists())

    def test_modified_manifest_anchor_is_rejected(self):
        name, folder = self.archive()
        value = json.loads((folder / 'manifest.json').read_text())
        value['started_at'] -= 1
        (folder / 'manifest.json').write_text(json.dumps(value))
        with self.assertRaises(backup.BackupError):
            self.store.validate(name)

    def test_links_and_unexpected_files_are_never_followed_or_deleted(self):
        name, folder = self.archive(backup.MAX_AGE)
        outside = self.root / 'user.txt'
        outside.write_text('keep')
        (folder / 'unexpected').symlink_to(outside)
        with self.assertRaises(backup.BackupError):
            self.store.prune()
        self.assertEqual(outside.read_text(), 'keep')
        with self.assertRaises(backup.BackupError):
            self.store.validate('../user.txt')

    def test_manifest_and_archive_symlinks_are_rejected(self):
        name, folder = self.archive()
        target = self.root / 'user.txt'
        target.write_text('{}')
        (folder / 'manifest.json').unlink()
        (folder / 'manifest.json').symlink_to(target)
        with self.assertRaises(backup.BackupError):
            self.store.validate(name)

    def test_store_symlink_and_lock_symlink_are_rejected(self):
        alias = self.root.parent / 'alias'
        alias.symlink_to(self.root, target_is_directory=True)
        with self.assertRaises(backup.BackupError):
            with backup.Store(alias, self.now):
                pass
        (self.root / '.lock').symlink_to(self.root.parent / 'outside')
        with self.assertRaises(backup.BackupError):
            with self.store:
                pass

    def test_fifo_lock_is_rejected_without_waiting_for_a_reader(self):
        os.mkfifo(self.root / '.lock')
        with self.assertRaises(backup.BackupError):
            with self.store:
                pass

    def test_concurrent_writer_is_rejected(self):
        with self.store:
            with self.assertRaises(backup.BackupError):
                with backup.Store(self.root, self.now):
                    pass

    def test_authenticated_decryption_must_finish_before_archive_inspection(self):
        name, _ = self.archive()
        with patch.object(backup, 'decrypt', side_effect=backup.BackupError), patch.object(backup, 'inspect_archive') as inspect:
            with self.assertRaises(backup.BackupError):
                self.store.verify(name, Path('/unused'))
            inspect.assert_not_called()

    def test_failed_dump_does_not_publish(self):
        recipient = self.root.parent / 'recipient'
        recipient.write_text('fixture')
        with patch.object(backup, 'encrypt_dump', side_effect=backup.BackupError):
            with self.assertRaises(backup.BackupError):
                self.store.backup(recipient, {})
        self.assertEqual(list(self.root.iterdir()), [])


if __name__ == '__main__':
    unittest.main()
