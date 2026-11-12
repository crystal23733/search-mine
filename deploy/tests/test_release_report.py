import sys
from pathlib import Path
import unittest
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
import release_report

class Report(unittest.TestCase):
    def record(self,index):
        return {'version':1,'profile':'release','target':'isolated_real_main_loopback','source_commit':'a'*40,'worktree_dirty':False,'duration_seconds':60,'ack_samples':500,'applied_ack_samples':450,'player_seconds':960,'workload':'mixed_open_flag_attack_v1','errors':0,'applied_attacks':10,'applied_ack_rtt_ms_p95':20,'attack_rtt_ms_p95':20,'requested_pairs':8,'matched_players':16,'ack_rtt_ms_p95':20,'run_id':str(index),'os':'linux','arch':'aarch64','logical_cpus':4,'limits':{}}
    def test_only_repeated_same_build_host_controls_produce_seventy_percent_candidate(self):
        records=[self.record(i) for i in range(3)]
        candidate=release_report.admission(records)
        self.assertEqual(candidate[0]['admission_pairs_at_most'],5)
        self.assertFalse(candidate[0]['maximum_capacity_found'])
        self.assertTrue(candidate[0]['target_hardware_review_required'])
    def test_failures_missing_samples_reused_ids_debug_dirty_and_other_hosts_never_qualify(self):
        for field,value in [('errors',1),('ack_samples',0),('applied_attacks',9),('attack_rtt_ms_p95',201),('applied_ack_rtt_ms_p95',float('nan')),('duration_seconds',8),('ack_rtt_ms_p95',float('nan')),('ack_rtt_ms_p95',201),('profile','debug'),('worktree_dirty',True),('matched_players',15),('source_commit',None),('os','different')]:
            records=[self.record(i) for i in range(3)]
            records[2][field]=value
            self.assertEqual(release_report.admission(records),[],field)
        self.assertEqual(release_report.admission([self.record(1)]*3),[])
