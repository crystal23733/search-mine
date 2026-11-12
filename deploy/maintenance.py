"""Bounded explicit backup/checkpoint jobs. Receipts contain no personal identifiers or secrets."""
import argparse
from datetime import datetime,timezone
import fcntl
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time


def run(job,env_file,state_dir,execute=subprocess.run):
    root=Path(__file__).resolve().parents[1]
    state=Path(state_dir)
    if state.is_symlink() or not state.is_dir() or state.stat().st_mode & 0o027: raise ValueError()
    with (state/'.lock').open('a') as lock:
        deadline=time.monotonic()+60
        while True:
            try:
                fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
                break
            except BlockingIOError:
                if time.monotonic()>=deadline: raise ValueError()
                time.sleep(0.1)
        base=['docker','compose','--env-file',str(env_file),'-f','deploy/compose.yml','-f','deploy/backup.compose.yml','-f','deploy/ledger.compose.yml']
        names={'backup':['backup','backup-prune'],'ledger':['ledger-capture','ledger-prune']}[job]
        steps={}
        # Prune must still run if creation fails, so expiry never depends on successful capture.
        for name in names:
            try:
                result=execute(base+['run','--rm','--no-deps',name],cwd=root,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,timeout=900,check=False)
                steps[name]='passed' if result.returncode==0 else 'failed'
            except (OSError,subprocess.SubprocessError): steps[name]='failed'
        receipt={'version':1,'job':job,'completed_at':datetime.now(timezone.utc).isoformat(),'status':'passed' if all(s=='passed' for s in steps.values()) else 'failed','steps':steps}
        temporary=None
        try:
            with tempfile.NamedTemporaryFile(mode='w',dir=state,delete=False) as stream:
                temporary=Path(stream.name)
                json.dump(receipt,stream);stream.flush();os.fsync(stream.fileno())
            os.replace(temporary,state/(job+'.json'))
            fd=os.open(state,os.O_DIRECTORY|os.O_RDONLY)
            try: os.fsync(fd)
            finally: os.close(fd)
        finally:
            if temporary and temporary.exists(): temporary.unlink()
        return receipt


def status(state_dir,now=None):
    now=now or datetime.now(timezone.utc)
    checks={}
    for job,age in [('backup',24*3600),('ledger',2*3600)]:
        try:
            path=Path(state_dir)/(job+'.json')
            if path.is_symlink() or not 0<path.stat().st_size<=4096: raise ValueError()
            value=json.loads(path.read_text())
            at=datetime.fromisoformat(value['completed_at'])
            checks[job]='passed' if value.get('status')=='passed' and value.get('job')==job and at.tzinfo is not None and 0<=(now-at).total_seconds()<=age else 'failed'
        except (OSError,ValueError,KeyError,TypeError): checks[job]='failed'
    return {'status':'passed' if all(v=='passed' for v in checks.values()) else 'failed','jobs':checks}


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('job',choices=['backup','ledger','status'])
    parser.add_argument('--env-file')
    parser.add_argument('--state-dir',required=True)
    args=parser.parse_args();os.umask(0o077)
    try:
        if args.job=='status': receipt=status(args.state_dir)
        else:
            if not args.env_file: raise ValueError()
            receipt=run(args.job,args.env_file,args.state_dir)
    except (OSError,ValueError): receipt={'status':'failed','code':'maintenance_unavailable'}
    print(json.dumps(receipt))
    return 0 if receipt['status']=='passed' else 1
if __name__=='__main__': raise SystemExit(main())
