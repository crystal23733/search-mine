"""Bundle budgets and conservative admission candidates from repeated isolated observations."""
import argparse
import gzip
import json
import math
from pathlib import Path


def bundles(root):
    root=Path(root).resolve()
    manifest=json.loads((root/'.vite/manifest.json').read_text())
    initial=set()
    def visit(key):
        if key in initial: return
        initial.add(key)
        for child in manifest[key].get('imports',[]): visit(child)
    visit('index.html')
    def asset(name):
        path=(root/name).resolve()
        if not path.is_relative_to(root) or not path.is_file(): raise ValueError()
        data=path.read_bytes()
        return {'file':name,'bytes':len(data),'gzip_bytes':len(gzip.compress(data,mtime=0))}
    scripts={item['file'] for item in manifest.values() if item['file'].endswith('.js')}
    first={manifest[key]['file'] for key in initial if manifest[key]['file'].endswith('.js')}
    entries=[asset(name) for name in sorted(first)]
    size=sum(item['gzip_bytes'] for item in entries)
    return {'initial_scripts':entries,'initial_gzip_bytes':size,'initial_budget_bytes':200*1024,
            'initial_budget_passed':size<=200*1024,'lazy_scripts':[asset(name) for name in sorted(scripts-first)],
            'wasm':[asset(str(path.relative_to(root))) for path in sorted(root.rglob('*.wasm'))]}


def acceptable_latency(value):
    return type(value) in (int,float) and math.isfinite(value) and 0<=value<=200


def admission(observations):
    groups={}
    for record in observations:
        # A short smoke/debug run is never capacity evidence. Compare only identical controls/build/host.
        if (record.get('version')!=1 or record.get('profile')!='release'
            or record.get('target')!='isolated_real_main_loopback' or record.get('worktree_dirty',True)
            or record.get('duration_seconds',0)<60 or record.get('ack_samples',0)<100
            or record.get('applied_ack_samples',0)<100
            or record.get('player_seconds',0)<0.9*record.get('matched_players',0)*record.get('duration_seconds',0)
            or record.get('workload')!='mixed_open_flag_attack_v1'
            or record.get('errors')!=0 or record.get('applied_attacks',0)<10
            or record.get('matched_players')!=2*record.get('requested_pairs',0)
            or not all(acceptable_latency(record.get(key)) for key in ['ack_rtt_ms_p95','applied_ack_rtt_ms_p95','attack_rtt_ms_p95'])):
            continue
        key=json.dumps({k:record.get(k) for k in ['source_commit','os','arch','logical_cpus','limits','workload']},sort_keys=True)
        if not record.get('source_commit'): continue
        runs=groups.setdefault((key,record['requested_pairs']),{})
        runs[record.get('run_id')]=record
    passing=[(key,pairs) for (key,pairs),runs in groups.items() if len(runs)>=3 and None not in runs]
    return [{'context':json.loads(key),'tested_pairs':pairs,'admission_pairs_at_most':pairs*7//10,
             'maximum_capacity_found':False,'target_hardware_review_required':True} for key,pairs in sorted(passing)]


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--dist',default='apps/web/dist')
    parser.add_argument('--load',action='append',default=[])
    args=parser.parse_args()
    try:
        report={'bundle':bundles(args.dist),'admission_candidates':admission([json.loads(Path(p).read_text()) for p in args.load]),
                'global_rtt_measured':False,'internal_input_latency_measured':False,'operational_capacity_verified':False}
        print(json.dumps(report,indent=2))
        return 0 if report['bundle']['initial_budget_passed'] else 1
    except (OSError,ValueError,KeyError,TypeError):
        print('{"status":"failed","code":"release_report_invalid"}')
        return 1
if __name__=='__main__': raise SystemExit(main())
