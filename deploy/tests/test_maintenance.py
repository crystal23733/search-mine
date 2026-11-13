import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
import maintenance
class Maintenance(unittest.TestCase):
    def test_prune_runs_after_capture_failure_and_receipt_does_not_hide_failure(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp);root.chmod(0o700);calls=[]
            def execute(args,**kwargs):
                calls.append(args[-1])
                if len(calls)==1: raise subprocess.TimeoutExpired('private command',900)
                return subprocess.CompletedProcess(args,0)
            receipt=maintenance.run('ledger','/restricted/runtime.env',root,execute)
            self.assertEqual(calls,['ledger-capture','ledger-prune'])
            self.assertEqual(receipt['status'],'failed')
            self.assertEqual(json.loads((root/'ledger.json').read_text()),receipt)
            self.assertNotIn('private',json.dumps(receipt))

    def test_missing_and_stale_success_receipts_fail_status(self):
        with tempfile.TemporaryDirectory() as temp:
            self.assertEqual(maintenance.status(temp)['status'],'failed')
            for job in ['backup','ledger']:
                (Path(temp)/(job+'.json')).write_text(json.dumps({'job':job,'completed_at':'2000-01-01T00:00:00Z','status':'passed'}))
            self.assertEqual(maintenance.status(temp)['jobs'],{'backup':'failed','ledger':'failed'})
