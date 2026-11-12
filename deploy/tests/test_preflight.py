import copy
from datetime import datetime, timezone
import sys
from pathlib import Path
import tempfile
import unittest
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import preflight


class Preflight(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name).resolve()
        self.root.chmod(0o700)
        key = self.root/'key'
        key.write_text('synthetic; must never appear in output')
        key.chmod(0o400)
        image = 'sha256:'+'a'*64
        service = {'image': image, 'read_only': True, 'cap_drop': ['ALL']}
        names = ['postgres','migrate','grant','server','web','cloudflared','backup','ledger-capture','ledger-prune']
        self.config = {'services': {n:copy.deepcopy(service) for n in names}, 'networks': {'app':{'internal':True},'db':{'internal':True}}, 'secrets': {'key':{'file':str(key)}}}
        server = self.config['services']['server']
        server['secrets'] = [{'source':'key'}]
        server['environment'] = {'LIAR_PUBLIC_ORIGIN':'https://sweeper.org', **{f'LIAR_{p}_CLIENT_ID':'registered-public-id' for p in ['GOOGLE','APPLE','KAKAO','NAVER']}}
        self.config['services']['web']['build'] = {'args':{'VITE_PUBLIC_SITE_ORIGIN':'https://sweeper.org'}}
        for name,path in [('backup','/backups'),('ledger-capture','/ledger')]:
            self.config['services'][name]['volumes'] = [{'type':'bind','target':path,'source':str(self.root)}]
        self.now = datetime(2026,11,13,tzinfo=timezone.utc)
        self.evidence = {'version':1,'limits':{},'origin':'https://sweeper.org','images':{'server':image,'web':image},'checks':{k:{'status':'passed','performed_at':'2026-11-12T20:00:00Z','record':'operator restricted record reference'} for k in preflight.EVIDENCE}}

    def tearDown(self): self.temp.cleanup()
    def check(self): return preflight.check(self.config,self.evidence,self.now)
    def test_complete_private_release_is_ready(self): self.assertEqual(self.check(),{'status':'ready','checks':[]})
    def test_exposure_mutable_image_and_plaintext_are_blocked_without_values(self):
        self.config['services']['server'].update(ports=[{'published':'3000'}],image='server:latest')
        self.config['services']['server']['environment']['LIAR_GOOGLE_CLIENT_SECRET'] = 'very-private-value'
        report = self.check()
        self.assertIn('public_host_exposure',report['checks'])
        self.assertIn('immutable_images_required',report['checks'])
        self.assertIn('plaintext_secret_configuration',report['checks'])
        self.assertNotIn('very-private-value',str(report))
    def test_world_readable_empty_and_symlink_keys_are_blocked(self):
        key=self.root/'key'
        for mode in [0o444,0o640]:
            key.chmod(mode)
            self.assertEqual('secret_files_not_private' in self.check()['checks'],mode==0o444)
        key.write_text('')
        self.assertIn('secret_files_not_private',self.check()['checks'])
        key.unlink();key.symlink_to(self.root/'missing')
        self.assertIn('secret_files_not_private',self.check()['checks'])
    def test_stale_future_other_release_and_missing_evidence_block(self):
        self.evidence['checks']['pwa_real_devices']['performed_at']='2026-11-14T00:00:00Z'
        self.evidence['checks']['capacity_on_target']['performed_at']='2026-10-01T00:00:00Z'
        report=self.check()
        self.assertIn('operator_check_required:pwa_real_devices',report['checks'])
        self.assertIn('operator_check_required:capacity_on_target',report['checks'])
        self.evidence['images']['web']='sha256:'+'b'*64
        self.assertIn('release_evidence_binding_missing',self.check()['checks'])
    def test_public_origin_requires_confirmed_host(self):
        for value in ['http://sweeper.org','https://127.0.0.1','https://localhost','https://x.example','https://user:secret@sweeper.org','https://sweeper.org/','https://sweeper.org/path','https://[::1]']:
            self.assertFalse(preflight.public_origin(value))
