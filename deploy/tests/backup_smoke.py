"""Actual PG18/age round trip, only in a newly owned and destroyed fixture."""
import json
import shutil
import time
import uuid

from smoke import ROOT, Smoke, require


class BackupSmoke(Smoke):
    def __init__(self):
        super().__init__(False)
        self.backup_dir = self.folder / 'backups'
        self.backup_dir.mkdir(mode=0o700)
        # Synthetic fixture only: host parent remains 0700; container UID is 10001.
        self.backup_dir.chmod(0o777)
        self.image = 'liar-backup:' + self.project
        with self.env.open('a') as stream:
            stream.write(f'LIAR_BACKUP_DIR={self.backup_dir}\nLIAR_BACKUP_IMAGE={self.image}\n')
        self.compose += ['-f', 'deploy/backup.compose.yml']

    def run(self, args, **kwargs):
        if args[:2] == ['docker', 'run']:
            # Override the inherited PG image VOLUME and track every one-off container.
            args = args[:2] + ['--label', 'com.docker.compose.project=' + self.project,
                               '--tmpfs', '/var/lib/postgresql:size=1m,mode=0700,uid=10001,gid=10001'] + args[2:]
        return super().run(args, **kwargs)

    def tool(self, service, *args, okay=True):
        return self.compose_run('run', '--rm', '--no-deps', service, *args, okay=okay, timeout=120)

    def fixture_sql(self, query, database='liar', okay=True):
        require(database in ['liar', 'restored_fixture', 'failed_fixture'], 'Unexpected fixture DB')
        script = ('PGPASSWORD=$(cat /run/secrets/db_admin_password); export PGPASSWORD; '
                  f'exec psql --no-psqlrc -h 127.0.0.1 -U postgres -d {database} -At -v ON_ERROR_STOP=1')
        return self.compose_run('exec', '-T', 'postgres', 'sh', '-c', script, data=query.encode(), okay=okay)

    def image_script(self, script, network='none', okay=True):
        args = ['docker', 'run', '--rm', '--network', network, '--read-only', '--cap-drop', 'ALL', '--security-opt', 'no-new-privileges',
                '--tmpfs', '/tmp:size=1g,mode=1777', '--mount', f'type=bind,src={self.backup_dir},dst=/backups,readonly',
                ]
        names = ['backup_identity', 'other_identity']
        if network != 'none':
            names.append('db_owner_password')
        for name in names:
            if (self.secret_dir / name).is_file():
                args += ['--mount', f'type=bind,src={self.secret_dir / name},dst=/run/secrets/{name},readonly']
        args += ['--entrypoint', 'python3', '-i', self.image, '-']
        return self.run(args, data=script.encode(), okay=okay, timeout=120)

    def exercise(self):
        self.stage('Building backup image and current migration binary')
        for dockerfile, target, image in [('deploy/backup.Dockerfile', 'backup', self.image),
                                           ('deploy/Dockerfile', 'server', self.server_image)]:
            self.run(['docker', 'build', '--target', target, '-f', dockerfile, '-t', image, '.'], timeout=1200)
        versions = self.image_script("import subprocess,json; print(json.dumps({x:subprocess.check_output([x,'--version']).decode().strip() for x in ['age','pg_dump','pg_restore']}))")
        self.results['versions'] = json.loads(versions.stdout)
        self.results['backup_image'] = json.loads(self.run(['docker', 'image', 'inspect', self.image]).stdout)[0]['Id']
        self.image_script("import os;assert os.getuid()==10001;assert not os.access('/app',os.W_OK)")
        for name in ['backup_identity', 'other_identity']:
            result = self.run(['docker', 'run', '--rm', '--network', 'none', '--entrypoint', 'age-keygen', self.image])
            key = result.stdout.decode()
            self.values[name] = key
            (self.secret_dir / name).write_text(key)
            (self.secret_dir / name).chmod(0o444)
        recipient = self.image_script("import subprocess; print(subprocess.check_output(['age-keygen','-y','/run/secrets/backup_identity']).decode(),end='')").stdout
        (self.secret_dir / 'backup_recipient').write_bytes(recipient)
        (self.secret_dir / 'backup_recipient').chmod(0o444)
        config = json.loads(self.compose_run('--profile', 'backup', '--profile', 'backup-verify', '--profile', 'backup-prune', 'config', '--format', 'json').stdout)
        for name in ['backup', 'backup-verify', 'backup-prune']:
            service = config['services'][name]
            require(service['user'] == '10001:10001' and service['read_only'] and 'ALL' in service['cap_drop'] and not service.get('ports'), 'Tool isolation mismatch')
        require(set(config['services']['backup']['networks']) == {'db'}, 'Backup must use only internal DB network')
        require(config['services']['backup-verify']['network_mode'] == 'none', 'Verifier must have no network')
        require({s['source'] for s in config['services']['backup']['secrets']} == {'db_owner_password', 'backup_recipient'}, 'Backup must not mount identity or vault keys')
        self.results['configuration'] = True
        self.compose_run('up', '-d', '--wait', '--wait-timeout', '120', 'postgres')
        self.compose_run('run', '--rm', 'migrate', timeout=120)
        self.fixture_sql("""
INSERT INTO auth_accounts VALUES('11111111-1111-4111-8111-111111111111','Backup fixture','2026-10-01T00:00:00Z','2026-10-02T00:00:00Z');
INSERT INTO auth_identities(id,account_id,provider,issuer,subject_digest,digest_key_version,linked_at)
VALUES('22222222-2222-4222-8222-222222222222','11111111-1111-4111-8111-111111111111','google','https://accounts.google.com',decode(repeat('ab',32),'hex'),1,'2026-10-01T00:00:00Z');
INSERT INTO auth_deletion_tombstones VALUES('33333333-3333-4333-8333-333333333333','2026-11-01T00:00:00Z','2026-11-29T00:00:00Z');
INSERT INTO metric_collection VALUES(true,1700000000,1700000060);
INSERT INTO product_metric_aggregates(metric,period,computed_at,observed_through,expires_at,complete,sample_size,retained)
VALUES('d7','2026-10-01',1791417600,1791417600,1825545600,true,2,1);
""")
        self.stage('Streaming encrypted dump and full offline verification')
        name = json.loads(self.tool('backup').stdout)['name']
        listing = self.image_script("import pathlib,json;p=pathlib.Path('/backups/" + name + "');print(json.dumps(sorted(x.name for x in p.iterdir())))").stdout
        require(json.loads(listing) == ['database.dump.age', 'manifest.json'], 'Unexpected backup output')
        original = self.image_script("import pathlib,sys;sys.stdout.buffer.write(pathlib.Path('/backups/" + name + "/database.dump.age').read_bytes())").stdout
        require(original.startswith(b'age-encryption.org/'), 'Archive was not age ciphertext')
        require(b'Backup fixture' not in original, 'Plaintext fixture appeared in encrypted archive')
        self.tool('backup-verify', 'verify', '--name', name)
        self.results['encrypted_backup_and_verify'] = True
        self.fixture_sql('CREATE DATABASE restored_fixture OWNER liar_owner; CREATE DATABASE failed_fixture OWNER liar_owner;')
        self.fixture_sql('DROP SCHEMA public; CREATE SCHEMA public AUTHORIZATION liar_owner;', 'restored_fixture')
        self.fixture_sql('DROP SCHEMA public; CREATE SCHEMA public AUTHORIZATION liar_owner;', 'failed_fixture')
        template = """import sys,subprocess
sys.path.insert(0,'/app')
import backup
store=backup.Store('/backups',int(__import__('time').time()))
encrypted=store.validate(__ARCHIVE__)
with backup.decrypt(encrypted, __import__('pathlib').Path('/run/secrets/backup_identity')) as clear:
 env=backup.pg_environment(); env['PGDATABASE']=__DB__
 result=subprocess.run(['pg_restore','--no-password','--single-transaction','--exit-on-error','--no-owner','--no-privileges','--dbname='+__DB__],stdin=clear,env=env,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
 raise SystemExit(result.returncode)
"""
        restore = lambda archive, db: template.replace('__ARCHIVE__', repr(archive)).replace('__DB__', repr(db))
        started = time.monotonic()
        self.image_script(restore(name, 'restored_fixture'), self.project + '_db')
        self.results['fixture_restore_seconds'] = round(time.monotonic() - started, 3)
        snapshot = """SELECT version,encode(checksum,'hex') FROM _sqlx_migrations ORDER BY version;
SELECT row_to_json(a)::text FROM auth_accounts a ORDER BY id;
SELECT row_to_json(i)::text FROM auth_identities i ORDER BY id;
SELECT row_to_json(t)::text FROM auth_deletion_tombstones t ORDER BY account_id;
SELECT row_to_json(m)::text FROM metric_collection m;
SELECT row_to_json(g)::text FROM product_metric_aggregates g ORDER BY metric,period;
SELECT count(*) FROM pg_constraint WHERE contype='f';"""
        require(self.fixture_sql(snapshot).stdout == self.fixture_sql(snapshot, 'restored_fixture').stdout, 'Restored schema/rows/retention anchors changed')
        require(self.fixture_sql("INSERT INTO auth_identities(id,account_id,provider,issuer,subject_digest,digest_key_version,linked_at) VALUES(gen_random_uuid(),gen_random_uuid(),'google','https://accounts.google.com',decode(repeat('cd',32),'hex'),1,now());", 'restored_fixture', okay=False).returncode != 0, 'Restored FK was not enforced')
        require(self.fixture_sql('UPDATE product_metric_aggregates SET expires_at=expires_at+1;', 'restored_fixture', okay=False).returncode != 0, 'Restored aggregate expiry was mutable')
        require(self.fixture_sql('UPDATE metric_collection SET coverage_since=started_at;', 'restored_fixture', okay=False).returncode != 0, 'Restored metric coverage regressed')
        self.results['empty_database_round_trip'] = True
        self.stage('Rejecting authenticated ciphertext damage, wrong identity and dump failures')
        damaged = 'backup-' + str(int(time.time())) + '-' + uuid.uuid4().hex
        # Host owner differs from container UID; copies remain inside the 0700 fixture.
        self.run(['docker', 'run', '--rm', '--network', 'none', '--mount', f'type=bind,src={self.backup_dir},dst=/backups', '--entrypoint', 'python3', self.image, '-c', "import shutil;shutil.copytree('/backups/" + name + "','/backups/" + damaged + "')"])
        for body in [original[:-1], original[:-1] + bytes([original[-1] ^ 1])]:
            # Recompute checksum to exercise age authentication beyond the external checksum.
            modify = "import pathlib,json,hashlib;p=pathlib.Path('/backups/" + damaged + "');b=bytes.fromhex('" + body.hex() + "');(p/'database.dump.age').write_bytes(b);m=json.loads((p/'manifest.json').read_text());m['started_at']=" + damaged.split('-')[1] + ";m.update(size=len(b),sha256=hashlib.sha256(b).hexdigest());(p/'manifest.json').write_text(json.dumps(m))"
            self.run(['docker', 'run', '--rm', '--network', 'none', '--mount', f'type=bind,src={self.backup_dir},dst=/backups', '--entrypoint', 'python3', '-i', self.image, '-'], data=modify.encode())
            result = self.tool('backup-verify', 'verify', '--name', damaged, okay=False)
            require(result.returncode != 0 and b'backup_operation_failed' in result.stdout, 'Corruption was accepted')
            require(self.image_script(restore(damaged, 'failed_fixture'), self.project + '_db', okay=False).returncode != 0, 'Corrupt data reached a successful restore')
        require(self.fixture_sql("SELECT count(*) FROM pg_tables WHERE schemaname='public';", 'failed_fixture').stdout.strip() == b'0', 'Authentication failure wrote DB tables')
        wrong_key = "import sys;sys.path.insert(0,'/app');import backup;from pathlib import Path;backup.Store('/backups',int(__import__('time').time())).verify(" + repr(name) + ",Path('/run/secrets/other_identity'))"
        require(self.image_script(wrong_key, okay=False).returncode != 0, 'Wrong private key was accepted')
        before = set(self.backup_dir.iterdir())
        # Real pg_dump authentication failure can still yield successful age output: require producer failure.
        password = self.secret_dir / 'db_owner_password'
        password.chmod(0o644)
        password.write_text('invalid-synthetic-password')
        password.chmod(0o444)
        require(self.tool('backup', okay=False).returncode != 0, 'Failed pg_dump was published')
        password.chmod(0o644)
        password.write_text(self.values['db_owner_password'])
        password.chmod(0o444)
        public = self.secret_dir / 'backup_recipient'
        public.chmod(0o644)
        public.write_text('invalid-recipient')
        public.chmod(0o444)
        require(self.tool('backup', okay=False).returncode != 0, 'Failed encryptor was published')
        require(before == set(self.backup_dir.iterdir()), 'Failed backup left pending/completed files')
        self.results['corruption_wrong_key_and_pipeline_failures'] = True
        self.results['status'] = 'passed'

    def cleanup(self):
        try:
            # Remove only the newly owned encrypted fixture archive, via its container UID.
            self.run(['docker', 'run', '--rm', '--network', 'none', '--mount', f'type=bind,src={self.backup_dir},dst=/backups', '--entrypoint', 'python3', self.image, '-c', "import pathlib,shutil;p=pathlib.Path('/backups');[(shutil.rmtree(x) if x.is_dir() else x.unlink()) for x in p.iterdir()]"] , okay=False)
            shutil.rmtree(self.backup_dir)
        finally:
            # Include interrupted one-off docker run calls, not just Compose services.
            owned = self.run(['docker', 'ps', '-aq', '--filter', 'label=com.docker.compose.project=' + self.project], okay=False)
            for cid in owned.stdout.decode().split():
                self.run(['docker', 'rm', '--force', '--volumes', cid], okay=False)
            super().cleanup()


def main():
    (ROOT / '.tmp').mkdir(exist_ok=True)
    smoke = BackupSmoke()
    try:
        smoke.exercise()
        print('Encrypted backup smoke PASS', flush=True)
    finally:
        smoke.cleanup()


if __name__ == '__main__':
    main()
