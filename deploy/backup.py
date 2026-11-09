"""Operator-only encrypted archives. No service restore or personal download API."""
import argparse
import contextlib
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import stat
import subprocess
import tempfile
import time
import uuid

MAX_AGE = 28 * 24 * 60 * 60
NAME = re.compile(r'(backup|pending)-([1-9][0-9]{0,11})-([0-9a-f]{32})\Z')
FILES = {'manifest.json', 'database.dump.age'}
TIMEOUT = 3600


class BackupError(Exception):
    pass


def regular(path):
    if not stat.S_ISREG(path.lstat().st_mode):
        raise BackupError()
    return path


def encrypt_dump(output, recipient, env):
    processes = []
    try:
        dump = subprocess.Popen(['pg_dump', '--format=custom', '--no-password', '--lock-wait-timeout=5s',
                                 '--exclude-table-data=service_recovery_control'],
                                env=env, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
        processes.append(dump)
        age = subprocess.Popen(['age', '--encrypt', '--recipients-file', str(regular(recipient))],
                               stdin=dump.stdout, stdout=output, stderr=subprocess.DEVNULL)
        processes.append(age)
        dump.stdout.close()
        # Both exit codes matter: a successfully encrypted partial dump is not a backup.
        codes = [process.wait(timeout=TIMEOUT) for process in processes]
        if any(codes):
            raise BackupError()
    finally:
        for process in processes:
            if process.poll() is None:
                process.kill()
            process.wait()


@contextlib.contextmanager
def decrypt(encrypted, identity):
    # The image mounts /tmp as tmpfs; no plaintext is written to the backup volume.
    with tempfile.TemporaryFile() as clear:
        with regular(encrypted).open('rb') as source:
            result = subprocess.run(['age', '--decrypt', '--identity', str(regular(identity))],
                                    stdin=source, stdout=clear, stderr=subprocess.DEVNULL, timeout=TIMEOUT)
        if result.returncode:
            raise BackupError()
        clear.seek(0)
        yield clear


def inspect_archive(clear):
    result = subprocess.run(['pg_restore', '--list'], stdin=clear, stdout=subprocess.DEVNULL,
                            stderr=subprocess.DEVNULL, timeout=TIMEOUT)
    if result.returncode:
        raise BackupError()


def digest(path):
    with regular(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


class Store:
    def __init__(self, root, now):
        self.root = Path(root)
        self.now = now
        self.lock = None

    def __enter__(self):
        if not stat.S_ISDIR(self.root.lstat().st_mode):
            raise BackupError()
        try:
            self.lock = os.open(self.root / '.lock', os.O_WRONLY | os.O_CREAT | os.O_NOFOLLOW | os.O_NONBLOCK, 0o600)
            if not stat.S_ISREG(os.fstat(self.lock).st_mode):
                raise BackupError()
            fcntl.flock(self.lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except (OSError, BackupError) as error:
            self.__exit__(None, None, None)
            raise BackupError() from error
        return self

    def __exit__(self, *_):
        if self.lock is not None:
            os.close(self.lock)
            self.lock = None

    def metadata(self, name):
        match = NAME.fullmatch(name)
        if not match:
            raise BackupError()
        folder = self.root / name
        if not stat.S_ISDIR(folder.lstat().st_mode):
            raise BackupError()
        paths = list(folder.iterdir())
        if any(p.name not in FILES or not stat.S_ISREG(p.lstat().st_mode) for p in paths):
            raise BackupError()
        manifest = regular(folder / 'manifest.json')
        if manifest.stat().st_size > 1024:
            raise BackupError()
        value = json.loads(manifest.read_text())
        start = int(match[2])
        if (not isinstance(value, dict) or type(value.get('version')) is not int or value['version'] != 1
                or type(value.get('started_at')) is not int or value['started_at'] != start or start > self.now):
            raise BackupError()
        expected = {'version', 'started_at'}
        if match[1] == 'backup' or set(value) == {'version', 'started_at', 'size', 'sha256'}:
            expected |= {'size', 'sha256'}
            if ({p.name for p in paths} != FILES or type(value.get('size')) is not int or value['size'] <= 0
                    or not isinstance(value.get('sha256'), str) or not re.fullmatch('[0-9a-f]{64}', value['sha256'])):
                raise BackupError()
        if set(value) != expected:
            raise BackupError()
        return folder, value, self.now - start >= MAX_AGE

    def validate(self, name):
        if not name.startswith('backup-'):
            raise BackupError()
        folder, value, expired = self.metadata(name)
        encrypted = folder / 'database.dump.age'
        if expired or encrypted.stat().st_size != value['size'] or digest(encrypted) != value['sha256']:
            raise BackupError()
        return encrypted

    def prune(self):
        expired = []
        # Validate the entire candidate set before deleting any managed directory.
        for path in sorted(self.root.iterdir()):
            if path.name.startswith(('backup-', 'pending-')):
                folder, _, old = self.metadata(path.name)
                if old:
                    expired.append(folder)
        for folder in expired:
            for path in folder.iterdir():
                path.unlink()
            folder.rmdir()
        return len(expired)

    def backup(self, recipient, env):
        self.prune()
        suffix = f'{self.now}-{uuid.uuid4().hex}'
        pending = self.root / ('pending-' + suffix)
        pending.mkdir(mode=0o700)
        published = False
        try:
            manifest = {'version': 1, 'started_at': self.now}
            metadata = pending / 'manifest.json'
            metadata.write_text(json.dumps(manifest) + '\n')
            encrypted = pending / 'database.dump.age'
            with encrypted.open('xb') as output:
                encrypt_dump(output, recipient, env)
                output.flush()
                os.fsync(output.fileno())
            manifest.update(size=encrypted.stat().st_size, sha256=digest(encrypted))
            if manifest['size'] <= 0:
                raise BackupError()
            with metadata.open('w') as stream:
                json.dump(manifest, stream)
                stream.write('\n')
                stream.flush()
                os.fsync(stream.fileno())
            self.sync(pending)
            name = 'backup-' + suffix
            pending.rename(self.root / name)
            self.sync(self.root)
            published = True
            return name
        finally:
            if not published and pending.exists():
                for path in pending.iterdir():
                    path.unlink()
                pending.rmdir()

    @staticmethod
    def sync(folder):
        fd = os.open(folder, os.O_RDONLY | os.O_DIRECTORY)
        try:
            os.fsync(fd)
        finally:
            os.close(fd)

    def verify(self, name, identity):
        encrypted = self.validate(name)
        with decrypt(encrypted, identity) as clear:
            inspect_archive(clear)


def pg_environment():
    # Fixed Compose connection fields; never use a credential-bearing URI in argv.
    env = {key: value for key, value in os.environ.items() if not key.startswith('PG')}
    password = regular(Path('/run/secrets/db_owner_password')).read_bytes()
    if not 1 <= len(password) <= 16384 or b'\0' in password or b'\n' in password or b'\r' in password:
        raise BackupError()
    env.update(PGHOST='postgres', PGPORT='5432', PGDATABASE='liar', PGUSER='liar_owner',
               PGPASSWORD=password.decode(), PGCONNECT_TIMEOUT='5', PGSSLMODE='disable')
    return env


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=['backup', 'verify', 'prune'])
    parser.add_argument('--name')
    args = parser.parse_args()
    os.umask(0o077)
    def interrupted(*_):
        raise BackupError()

    signal.signal(signal.SIGTERM, interrupted)
    signal.signal(signal.SIGINT, interrupted)
    try:
        with Store('/backups', int(time.time())) as store:
            if args.command == 'backup':
                name = store.backup(Path('/run/secrets/backup_recipient'), pg_environment())
                print(json.dumps({'status': 'created', 'name': name}))
            elif args.command == 'verify':
                if not args.name:
                    raise BackupError()
                store.verify(args.name, Path('/run/secrets/backup_identity'))
                print('{"status":"verified"}')
            else:
                print(json.dumps({'status': 'pruned', 'removed': store.prune()}))
    except (BackupError, OSError, ValueError, subprocess.SubprocessError):
        print('{"status":"failed","error":"backup_operation_failed"}')
        return 1
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
