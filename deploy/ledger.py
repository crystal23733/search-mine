"""Trusted operator checkpoint; plaintext stays in memory, never in the store or logs."""
import argparse
import json
import os
from pathlib import Path
import re
import signal
import stat
import subprocess
import time
import uuid

from backup import BackupError, Store, digest, regular

MAX_BYTES = 128 * 1024
MAX_CIPHER = 1024 * 1024
MAX_AGE = 2419200
MAX_TIME = 253402300799
GEN = re.compile(r'(ledger|pending)-([0-9]{1,12})-([0-9a-f]{32})\Z')
PART = re.compile(r'part-([0-9]{1,12})-([0-9a-f]{32})\.age\Z')
TEMP = re.compile(r'current-([0-9a-f]{32})\.tmp\Z')


def require(condition):
    if not condition:
        raise BackupError()


def integer(value, low=0, high=MAX_TIME):
    require(type(value) is int and low <= value <= high)
    return value


def source_id(value):
    require(isinstance(value, str))
    parsed = uuid.UUID(value)
    require(parsed.int != 0 and str(parsed) == value)
    return value


def pairs(items):
    result = {}
    for key, value in items:
        require(key not in result)
        result[key] = value
    return result


def decode(data):
    require(len(data) <= MAX_BYTES)
    return json.loads(data, object_pairs_hook=pairs, parse_constant=lambda _: require(False))


def encode(value):
    data = json.dumps(value, separators=(',', ':'), allow_nan=False).encode()
    require(len(data) <= MAX_BYTES)
    return data


def read_json(path):
    require(regular(path).stat().st_size <= MAX_BYTES)
    return decode(path.read_bytes())


def write_json(path, value):
    with path.open('wb') as stream:
        stream.write(encode(value))
        stream.flush()
        os.fsync(stream.fileno())


def key_file(path):
    require(0 < regular(Path(path)).stat().st_size <= 16384)
    return path


def owner_environment():
    env = {k: v for k, v in os.environ.items() if not k.startswith('PG') and k not in {'DATABASE_URL', 'MIGRATION_DATABASE_URL'}}
    path = Path('/run/secrets/migration_database_url')
    key_file(path)
    data = path.read_bytes()
    require(0 < len(data) <= 16384 and not any(c in data for c in (b'\0', b'\r', b'\n')))
    env['MIGRATION_DATABASE_URL'] = data.decode()
    return env


def native(args, data, env, timeout):
    result = subprocess.run(['/app/bin/recovery', *args], input=data, env=env, stdout=subprocess.PIPE,
                            stderr=subprocess.DEVNULL, timeout=timeout)
    require(result.returncode == 0)
    return decode(result.stdout)


def encrypt(value, path, recipient, timeout):
    with path.open('xb') as stream:
        result = subprocess.run(['age', '--encrypt', '--recipients-file', str(key_file(recipient))],
                                input=encode(value), stdout=stream, stderr=subprocess.DEVNULL, timeout=timeout)
        require(result.returncode == 0)
        stream.flush()
        os.fsync(stream.fileno())
    require(0 < path.stat().st_size <= MAX_CIPHER)


def decrypt(path, identity, timeout):
    require(0 < regular(path).stat().st_size <= MAX_CIPHER)
    result = subprocess.run(['age', '--decrypt', '--identity', str(key_file(identity))],
                            input=path.read_bytes(), stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, timeout=timeout)
    # Authentication must finish before parsing or passing any plaintext to Rust/SQL.
    require(result.returncode == 0)
    return decode(result.stdout)


class LedgerStore:
    def __init__(self, root, clock=lambda: int(time.time())):
        self.root = Path(root)
        self.clock = clock
        self.last = 0
        self.deadline = time.monotonic() + 120
        self.guard = Store(root, 0)
        self.current_name = None

    def __enter__(self):
        self.guard.__enter__()
        return self

    def __exit__(self, *args):
        return self.guard.__exit__(*args)

    def now(self):
        now = integer(self.clock(), self.last)
        self.last = now
        self.remaining()
        return now

    def remaining(self):
        remaining = self.deadline - time.monotonic()
        require(remaining > 0)
        return remaining

    def pointer(self, now):
        path = self.root / 'current.json'
        if not path.exists() and not path.is_symlink():
            return None
        value = read_json(path)
        require(isinstance(value, dict) and set(value) == {'version', 'source_id', 'captured_at', 'name'})
        require(type(value['version']) is int and value['version'] == 1)
        source_id(value['source_id'])
        integer(value['captured_at'], 0, now)
        require(value['name'] is None or (isinstance(value['name'], str) and GEN.fullmatch(value['name']) and value['name'].startswith('ledger-')))
        self.current_name = value['name']
        return value

    def set_pointer(self, value):
        pending = self.root / ('current-' + uuid.uuid4().hex + '.tmp')
        try:
            write_json(pending, value)
            os.replace(pending, self.root / 'current.json')
            # Preserve the new generation if fsync fails after the pointer commit.
            self.current_name = value['name']
            Store.sync(self.root)
        finally:
            if pending.exists():
                pending.unlink()

    def manifest(self, folder, match, now):
        value = read_json(folder / 'manifest.json')
        require(isinstance(value, dict) and set(value) == {'version', 'source_id', 'captured_at', 'parts', 'index_size', 'index_sha256'})
        require(type(value['version']) is int and value['version'] == 1)
        source_id(value['source_id'])
        captured = integer(value['captured_at'], 0, now)
        require(captured == int(match[2]))
        require(isinstance(value['parts'], list) and len(value['parts']) <= 500)
        integer(value['index_size'], 1, MAX_CIPHER)
        require(isinstance(value['index_sha256'], str) and re.fullmatch('[0-9a-f]{64}', value['index_sha256']))
        names, expiries = set(), set()
        for part in value['parts']:
            require(isinstance(part, dict) and set(part) == {'name', 'expires_at', 'size', 'sha256'})
            require(isinstance(part['name'], str))
            item = PART.fullmatch(part['name'])
            require(item is not None and part['name'] not in names)
            names.add(part['name'])
            expires = integer(part['expires_at'], captured + 1, min(MAX_TIME, captured + MAX_AGE))
            require(expires == int(item[1]) and expires not in expiries)
            expiries.add(expires)
            integer(part['size'], 1, MAX_CIPHER)
            require(isinstance(part['sha256'], str) and re.fullmatch('[0-9a-f]{64}', part['sha256']))
        paths = {p.name for p in folder.iterdir()}
        require(paths <= names | {'manifest.json', 'index.age'})
        if now - captured <= MAX_AGE:
            require({'manifest.json', 'index.age'} <= paths)
            require(all(p['name'] in paths for p in value['parts'] if p['expires_at'] > now))
        return value

    def inventory(self, now):
        pointer = self.pointer(now)
        folders, temporary = [], []
        for path in self.root.iterdir():
            self.remaining()
            if path.name in {'.lock', 'current.json'}:
                regular(path)
                continue
            if TEMP.fullmatch(path.name):
                regular(path)
                temporary.append(path)
                continue
            match = GEN.fullmatch(path.name)
            require(match is not None and stat.S_ISDIR(path.lstat().st_mode))
            captured = integer(int(match[2]), 0, now)
            paths = list(path.iterdir())
            require(len(paths) <= 502)
            for item in paths:
                regular(item)
                part = PART.fullmatch(item.name)
                require(item.name in {'manifest.json', 'index.age'} or part is not None)
                if part:
                    integer(int(part[1]), captured + 1, min(MAX_TIME, captured + MAX_AGE))
            metadata = self.manifest(path, match, now) if match[1] == 'ledger' else None
            folders.append((path, captured, metadata))
        if pointer and pointer['name'] is not None:
            matching = [m for p, _, m in folders if p.name == pointer['name']]
            require(len(matching) == 1 and matching[0] is not None)
            require(all(matching[0][k] == pointer[k] for k in ['source_id', 'captured_at']))
        return pointer, folders, temporary

    def prune(self):
        now = self.now()
        pointer, folders, temporary = self.inventory(now)
        removed = 0
        if pointer and pointer['name'] and now - pointer['captured_at'] > MAX_AGE:
            pointer = dict(pointer, name=None)
            self.set_pointer(pointer)
        for path, _, metadata in folders:
            if not pointer or path.name != pointer['name']:
                for item in path.iterdir():
                    item.unlink()
                    removed += 1
                path.rmdir()
            else:
                for part in metadata['parts']:
                    item = path / part['name']
                    if part['expires_at'] <= now and item.exists():
                        item.unlink()
                        removed += 1
                Store.sync(path)
        for path in temporary:
            path.unlink()
        Store.sync(self.root)
        return removed

    def inspect(self, snapshot):
        env = {k: os.environ[k] for k in ['PATH', 'SystemRoot', 'WINDIR', 'TEMP', 'TMP', 'LLVM_PROFILE_FILE'] if k in os.environ}
        summary = native(['inspect-snapshot'], encode(snapshot), env, self.remaining())
        require(isinstance(summary, dict) and set(summary) == {'source_id', 'captured_at', 'entries'})
        require(set(snapshot) == {'version', 'source_id', 'captured_at', 'entries'})
        require(summary['source_id'] == snapshot['source_id'] and summary['captured_at'] == snapshot['captured_at'])
        integer(summary['entries'], 0, 500)
        require(isinstance(snapshot['entries'], list) and summary['entries'] == len(snapshot['entries']))
        self.now()
        return summary

    def capture(self, recipient, env):
        key_file(recipient)
        self.prune()
        snapshot = native(['capture'], None, env, self.remaining())
        summary = self.inspect(snapshot)
        pointer = self.pointer(self.now())
        if pointer:
            require(pointer['source_id'] == summary['source_id'] and pointer['captured_at'] <= summary['captured_at'])
        suffix = f"{summary['captured_at']}-{uuid.uuid4().hex}"
        pending = self.root / ('pending-' + suffix)
        completed = self.root / ('ledger-' + suffix)
        pending.mkdir(mode=0o700)
        renamed = False
        try:
            header = {'version': 1, 'source_id': summary['source_id'], 'captured_at': summary['captured_at']}
            write_json(pending / 'manifest.json', header)
            groups = {}
            for entry in snapshot['entries']:
                if entry['expires_at'] > self.now():
                    groups.setdefault(entry['expires_at'], []).append(entry)
            parts = []
            for expires, entries in sorted(groups.items()):
                if expires <= self.now():
                    continue
                name = f'part-{expires}-{uuid.uuid4().hex}.age'
                path = pending / name
                encrypt(dict(header, entries=entries), path, recipient, self.remaining())
                parts.append({'name': name, 'expires_at': expires, 'size': path.stat().st_size, 'sha256': digest(path)})
            index = dict(header, parts=parts)
            encrypt(index, pending / 'index.age', recipient, self.remaining())
            manifest = dict(index, index_size=(pending / 'index.age').stat().st_size, index_sha256=digest(pending / 'index.age'))
            write_json(pending / 'manifest.json', manifest)
            Store.sync(pending)
            pending.rename(completed)
            renamed = True
            Store.sync(self.root)
            self.set_pointer(dict(header, name=completed.name))
            self.prune()
            return summary
        finally:
            folder = completed if renamed else pending
            if folder.exists() and self.current_name != folder.name:
                for item in folder.iterdir():
                    regular(item).unlink()
                folder.rmdir()
                Store.sync(self.root)

    @staticmethod
    def check_file(path, size, sha256):
        require(regular(path).stat().st_size == size and digest(path) == sha256)

    def verified_snapshot(self, identity):
        key_file(identity)
        self.prune()
        now = self.now()
        pointer, folders, _ = self.inventory(now)
        require(pointer is not None and pointer['name'] is not None)
        folder, _, manifest = next(item for item in folders if item[0].name == pointer['name'])
        self.check_file(folder / 'index.age', manifest['index_size'], manifest['index_sha256'])
        index = decrypt(folder / 'index.age', identity, self.remaining())
        expected = {k: manifest[k] for k in ['version', 'source_id', 'captured_at', 'parts']}
        require(index == expected)
        header = {k: index[k] for k in ['version', 'source_id', 'captured_at']}
        snapshot = dict(header, entries=[])
        total = 0
        for part in index['parts']:
            if part['expires_at'] <= self.now():
                continue
            path = folder / part['name']
            self.check_file(path, part['size'], part['sha256'])
            clear = decrypt(path, identity, self.remaining())
            require(isinstance(clear, dict) and set(clear) == {'version', 'source_id', 'captured_at', 'entries'})
            require(all(clear[k] == header[k] for k in header))
            require(isinstance(clear['entries'], list) and 0 < len(clear['entries']) <= 500)
            require(all(isinstance(e, dict) and e.get('expires_at') == part['expires_at'] for e in clear['entries']))
            total += len(encode(clear))
            require(total <= MAX_BYTES * 2)
            snapshot['entries'].extend(clear['entries'])
            require(len(snapshot['entries']) <= 500)
        self.inspect(snapshot)
        # Reap entries that expired while the index was authenticated.
        self.prune()
        # A selected entry expiring mid-operation invalidates the verified set.
        require(all(e['expires_at'] > self.now() for e in snapshot['entries']))
        return snapshot

    def verify(self, identity):
        snapshot = self.verified_snapshot(identity)
        return {'captured_at': snapshot['captured_at'], 'entries': len(snapshot['entries'])}

    def audit(self, identity, env):
        snapshot = self.verified_snapshot(identity)
        result = native(['compare-snapshot'], encode(snapshot), env, self.remaining())
        require(isinstance(result, dict) and set(result) == {'captured_at', 'observed_at', 'entries'})
        integer(result['captured_at'], snapshot['captured_at'], snapshot['captured_at'])
        integer(result['observed_at'], snapshot['captured_at'], self.now())
        integer(result['entries'], 0, len(snapshot['entries']))
        self.prune()
        return result

    def apply(self, identity, incident, env):
        require(isinstance(incident, str) and uuid.UUID(incident).int != 0)
        snapshot = self.verified_snapshot(identity)
        result = native(['apply-snapshot', incident], encode(snapshot), env, self.remaining())
        require(isinstance(result, dict) and set(result) == {'state', 'applied'} and result['state'] == 'quarantined')
        integer(result['applied'], 0, 500)
        return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=['capture', 'verify', 'audit', 'apply', 'prune'])
    parser.add_argument('--incident')
    args = parser.parse_args()
    os.umask(0o077)

    def interrupted(*_):
        raise BackupError()

    signal.signal(signal.SIGINT, interrupted)
    signal.signal(signal.SIGTERM, interrupted)
    try:
        require(bool(args.incident) == (args.command == 'apply'))
        with LedgerStore('/ledger') as store:
            if args.command == 'capture':
                result = store.capture(Path('/run/secrets/backup_recipient'), owner_environment())
                print(json.dumps({'status': 'captured', 'captured_at': result['captured_at'], 'entries': result['entries']}))
            elif args.command == 'verify':
                print(json.dumps(dict(store.verify(Path('/run/secrets/backup_identity')), status='verified')))
            elif args.command == 'audit':
                print(json.dumps(dict(store.audit(Path('/run/secrets/backup_identity'), owner_environment()), status='matched')))
            elif args.command == 'apply':
                print(json.dumps(dict(store.apply(Path('/run/secrets/backup_identity'), args.incident, owner_environment()), status='applied')))
            else:
                print(json.dumps({'status': 'pruned', 'removed': store.prune()}))
    except (BackupError, OSError, ValueError, KeyError, TypeError, StopIteration, subprocess.SubprocessError):
        print('{"status":"failed","code":"ledger_operation_failed"}')
        return 1
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
