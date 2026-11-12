"""Actual age authentication and restored PostgreSQL sanitation, in owned fixtures only."""
import json
from urllib.parse import urlsplit

from backup_smoke import BackupSmoke
from smoke import ROOT, require

ACCOUNT = '11111111-1111-4111-8111-111111111111'
SURVIVOR = '44444444-4444-4444-8444-444444444444'
IDENTITY = '55555555-5555-4555-8555-555555555555'


class LedgerSmoke(BackupSmoke):
    def __init__(self):
        super().__init__()
        self.image = 'liar-ledger:' + self.project
        with self.env.open('a') as stream:
            stream.write(f'LIAR_LEDGER_DIR={self.backup_dir}\nLIAR_LEDGER_IMAGE={self.image}\n')
        self.compose += ['-f', 'deploy/ledger.compose.yml']

    def container(self, args, keys=(), network='none', owner='migration_database_url', data=None, okay=True):
        command = ['docker', 'run', '--rm', '--network', network, '--read-only', '--cap-drop', 'ALL',
                   '--security-opt', 'no-new-privileges', '--tmpfs', '/tmp:size=32m,mode=1777',
                   '--mount', f'type=bind,src={self.backup_dir},dst=/ledger']
        for key in keys:
            source = owner if key == 'migration_database_url' else key
            command += ['--mount', f'type=bind,src={self.secret_dir / source},dst=/run/secrets/{key},readonly']
        if args[0] == 'script':
            command += ['--entrypoint', 'python3', '-i', self.image, '-']
            program = "import sys;sys.path.insert(0,'/app')\nimport ledger,json,pathlib\n" + args[1]
            data = program.encode()
        else:
            command += [self.image, *args]
        return self.run(command, data=data, okay=okay, timeout=150)

    def exercise(self):
        self.stage('Building native ledger image and migration binary')
        for dockerfile, target, image in [('deploy/backup.Dockerfile', 'recovery-ledger', self.image),
                                          ('deploy/Dockerfile', 'server', self.server_image)]:
            self.run(['docker', 'build', '--target', target, '-f', dockerfile, '-t', image, '.'], timeout=1200)
        version = self.container(['script', "import subprocess;print(json.dumps({x:subprocess.check_output([x,'--version']).decode().strip() for x in ['age','pg_dump']}))"])
        self.results['versions'] = json.loads(version.stdout)
        self.results['ledger_image'] = json.loads(self.run(['docker', 'image', 'inspect', self.image]).stdout)[0]['Id']
        for name in ['backup_identity', 'other_identity']:
            value = self.run(['docker', 'run', '--rm', '--network', 'none', '--entrypoint', 'age-keygen', self.image]).stdout.decode()
            self.values[name] = value
            (self.secret_dir / name).write_text(value)
            (self.secret_dir / name).chmod(0o444)
        recipient = self.run(['docker', 'run', '--rm', '--network', 'none', '--entrypoint', 'age-keygen',
                              '--mount', f'type=bind,src={self.secret_dir / "backup_identity"},dst=/identity,readonly',
                              self.image, '-y', '/identity']).stdout
        (self.secret_dir / 'backup_recipient').write_bytes(recipient)
        (self.secret_dir / 'backup_recipient').chmod(0o444)
        profiles = ['ledger-capture', 'ledger-verify', 'ledger-prune', 'ledger-apply']
        flags = [arg for p in profiles for arg in ['--profile', p]]
        config = json.loads(self.compose_run(*flags, 'config', '--format', 'json').stdout)
        expected = {'ledger-capture': {'migration_database_url', 'backup_recipient'},
                    'ledger-verify': {'backup_identity'}, 'ledger-prune': set(),
                    'ledger-apply': {'migration_database_url', 'backup_identity'}}
        for name in profiles:
            service = config['services'][name]
            require(service['user'] == '10001:10001' and service['read_only'] and not service.get('ports') and 'ALL' in service['cap_drop'], 'Ledger job isolation mismatch')
            require({s['source'] for s in service.get('secrets', [])} == expected[name], 'Ledger job has excessive keys')
            if name in ['ledger-verify', 'ledger-prune']:
                require(service['network_mode'] == 'none', 'Offline jobs must not have a network')
            else:
                require(set(service['networks']) == {'db'}, 'Owner jobs must only use the internal DB network')
        self.results['key_and_network_isolation'] = True
        self.compose_run('up', '-d', '--wait', '--wait-timeout', '120', 'postgres')
        self.compose_run('run', '--rm', 'migrate', timeout=120)
        self.fixture_sql(f"""
INSERT INTO auth_accounts VALUES('{ACCOUNT}','Erased fixture',now()-interval '28 days',now()),('{SURVIVOR}','Surviving fixture',now()-interval '28 days',now());
INSERT INTO auth_identities(id,account_id,provider,issuer,subject_digest,digest_key_version,linked_at)
VALUES('22222222-2222-4222-8222-222222222222','{ACCOUNT}','google','https://accounts.google.com',decode(repeat('ab',32),'hex'),1,now()-interval '28 days'),
('{IDENTITY}','{SURVIVOR}','google','https://accounts.google.com',decode(repeat('cd',32),'hex'),1,now()-interval '28 days');
CREATE DATABASE restored_fixture OWNER liar_owner;
""")
        dump = self.compose_run('exec', '-T', 'postgres', 'sh', '-c',
            'PGPASSWORD=$(cat /run/secrets/db_owner_password); export PGPASSWORD; exec pg_dump --format=custom --no-password --exclude-table-data=service_recovery_control -h 127.0.0.1 -U liar_owner -d liar').stdout
        self.compose_run('exec', '-T', 'postgres', 'sh', '-c',
            'PGPASSWORD=$(cat /run/secrets/db_owner_password); export PGPASSWORD; exec pg_restore --no-owner --no-privileges --exit-on-error -h 127.0.0.1 -U liar_owner -d restored_fixture', data=dump)
        require(self.fixture_sql('SELECT count(*) FROM service_recovery_control;', 'restored_fixture').stdout.strip() == b'0', 'Restored gate was copied')
        self.values['target_url'] = urlsplit(self.values['migration_database_url'])._replace(path='/restored_fixture').geturl()
        (self.secret_dir / 'target_url').write_text(self.values['target_url'])
        (self.secret_dir / 'target_url').chmod(0o444)
        network = self.project + '_db'
        incident = self.container(['script', "print(json.dumps(ledger.native(['quarantine'],None,ledger.owner_environment(),10)))"],
                                  keys=['migration_database_url'], owner='target_url', network=network)
        incident = json.loads(incident.stdout)['recovery_id']
        now = int(self.fixture_sql('SELECT floor(extract(epoch FROM clock_timestamp()))::bigint;').stdout)
        self.fixture_sql(f"""BEGIN;
INSERT INTO auth_deletion_tombstones VALUES('{ACCOUNT}',to_timestamp({now-10}),to_timestamp({now-10+2419200}));
DELETE FROM auth_accounts WHERE id='{ACCOUNT}';
INSERT INTO auth_identity_tombstones VALUES('{IDENTITY}',to_timestamp({now-2418900}),to_timestamp({now+300}));
DELETE FROM auth_identities WHERE id='{IDENTITY}';
COMMIT;""")
        self.stage('Capturing committed rights using only the public recipient')
        captured = json.loads(self.compose_run('run', '--rm', '--no-deps', 'ledger-capture').stdout)
        require(captured['entries'] == 2, 'Capture did not include both rights')
        verified = json.loads(self.compose_run('run', '--rm', '--no-deps', 'ledger-verify').stdout)
        require(verified['entries'] == 2 and verified['captured_at'] == captured['captured_at'], 'Offline verification changed the original snapshot')
        self.container(['script', f"""
root=pathlib.Path('/ledger')
for p in root.rglob('*'):
 if p.is_file():
  data=p.read_bytes()
  assert all(v.encode() not in data for v in ['{ACCOUNT}','{IDENTITY}','Erased fixture'])
print('no personal plaintext')
"""])
        self.results['public_recipient_capture_and_offline_authentication'] = True
        self.results['no_personal_plaintext_in_store'] = True
        self.stage('Rejecting the final ciphertext authentication failure before SQL')
        self.container(['script', f"""
root=pathlib.Path('/ledger');pointer=ledger.read_json(root/'current.json');folder=root/pointer['name'];manifest=ledger.read_json(folder/'manifest.json')
last=folder/manifest['parts'][-1]['name'];paths=[last,folder/'index.age',folder/'manifest.json'];original={{p:p.read_bytes() for p in paths}}
try:
 last.unlink()
 try:
  with ledger.LedgerStore(root) as store: store.apply(pathlib.Path('/run/secrets/backup_identity'),'{incident}',ledger.owner_environment())
 except ledger.BackupError: pass
 else: raise AssertionError('Missing final part reached SQL')
 last.write_bytes(original[last])
 body=last.read_bytes();last.write_bytes(body[:-1]+bytes([body[-1]^1]))
 manifest['parts'][-1]['sha256']=ledger.digest(last)
 index={{k:manifest[k] for k in ['version','source_id','captured_at','parts']}}
 (folder/'index.age').unlink();ledger.encrypt(index,folder/'index.age',pathlib.Path('/run/secrets/backup_recipient'),10)
 manifest['index_size']=(folder/'index.age').stat().st_size;manifest['index_sha256']=ledger.digest(folder/'index.age');ledger.write_json(folder/'manifest.json',manifest)
 try:
  with ledger.LedgerStore(root) as store: store.apply(pathlib.Path('/run/secrets/backup_identity'),'{incident}',ledger.owner_environment())
 except ledger.BackupError: pass
 else: raise AssertionError('Unauthenticated final part reached SQL')
finally:
 for p,body in original.items():p.write_bytes(body)
print('blocked before SQL')
"""], keys=['migration_database_url', 'backup_identity', 'backup_recipient'], owner='target_url', network=network)
        counts = self.fixture_sql('SELECT (SELECT count(*) FROM auth_accounts),(SELECT count(*) FROM auth_identities);', 'restored_fixture').stdout.strip()
        require(counts == b'2|2', 'Authentication failure changed the restored target')
        wrong = self.container(['script', "\ntry:\n with ledger.LedgerStore('/ledger') as s:s.verify(pathlib.Path('/run/secrets/other_identity'))\nexcept ledger.BackupError:pass\nelse:raise AssertionError('Wrong identity accepted')\n"], keys=['other_identity'])
        require(wrong.returncode == 0, 'Wrong key test failed')
        self.results['wrong_key_and_final_part_auth_failure_leave_db_unchanged'] = True
        self.stage('Applying the fully authenticated snapshot to the quarantined restore')
        for _ in range(2):
            result = self.container(['apply', '--incident', incident], keys=['migration_database_url', 'backup_identity'], owner='target_url', network=network)
            require(json.loads(result.stdout) == {'status': 'applied', 'state': 'quarantined', 'applied': 2}, 'Sanitation was not idempotent/quarantined')
        counts = self.fixture_sql('SELECT (SELECT count(*) FROM auth_accounts),(SELECT count(*) FROM auth_identities),(SELECT state FROM service_recovery_control);', 'restored_fixture').stdout.strip()
        require(counts == b'1|0|quarantined', 'Restored erasure/identity rights or quarantine were lost')
        self.results['restored_rights_applied_idempotently_without_activation'] = True
        self.stage('Pruning by original record expiry without recapturing timestamps')
        self.container(['script', f"""
root=pathlib.Path('/ledger')
with ledger.LedgerStore(root,lambda:{now+300}) as store:
 summary=store.verify(pathlib.Path('/run/secrets/backup_identity'))
 assert summary=={{'captured_at':{captured['captured_at']},'entries':1}}
 assert len(list(root.glob('ledger-*/part-*.age')))==1
with ledger.LedgerStore(root,lambda:{captured['captured_at']+2419201}) as store:store.prune()
assert not list(root.glob('ledger-*'));assert ledger.read_json(root/'current.json')['name'] is None
print('original expiry retained')
"""], keys=['backup_identity'])
        self.results['original_individual_expiry_and_generation_cleanup'] = True
        self.results['status'] = 'passed'


def main():
    (ROOT / '.tmp').mkdir(exist_ok=True)
    smoke = LedgerSmoke()
    try:
        smoke.exercise()
        print('Encrypted rights ledger smoke PASS', flush=True)
    finally:
        smoke.cleanup()


if __name__ == '__main__':
    main()
