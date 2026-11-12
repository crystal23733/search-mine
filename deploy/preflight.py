"""Read-only public-release checks. Never prints configuration values or secret contents."""
import argparse
from datetime import datetime, timezone
import json
from pathlib import Path
import re
import stat
import subprocess
from urllib.parse import urlsplit

ROOT = Path(__file__).resolve().parents[1]
IMMUTABLE = re.compile(r'(?:sha256:[0-9a-f]{64}|[^\s]+@sha256:[0-9a-f]{64})\Z')
EVIDENCE = ('oauth_four_providers', 'policies_and_cmp', 'ads_approval_and_isolation',
            'tunnel_and_network', 'backup_restore_and_previous_release',
            'capacity_on_target', 'pwa_real_devices', 'language_and_accessibility')


def public_origin(value):
    try:
        url = urlsplit(value)
        host = url.hostname or ''
        return (url.scheme == 'https' and value == f'https://{url.netloc}'
                and url.username is None and url.password is None and url.port in (None, 443)
                and '.' in host and not host.endswith(('.example', '.invalid', '.test', '.localhost'))
                and host != 'localhost' and not re.fullmatch(r'[0-9.]+', host) and ':' not in host)
    except (ValueError, TypeError):
        return False


def private_file(path):
    try:
        path = Path(path)
        if not path.is_absolute() or path.is_symlink() or path.resolve().is_relative_to(ROOT):
            return False
        info = path.stat()
        if not stat.S_ISREG(info.st_mode) or not 0 < info.st_size <= 16384 or stat.S_IMODE(info.st_mode) & 0o037:
            return False
        # Parent restrictions matter for traversal and replacement, not just the leaf mode.
        for parent in path.parents:
            if parent.is_symlink():
                return False
        parent = path.parent.stat()
        return not (stat.S_IMODE(parent.st_mode) & 0o027)
    except (OSError, TypeError):
        return False


def check(config, evidence, now=None):
    errors = set()
    now = now or datetime.now(timezone.utc)
    services = config.get('services', {})
    required = {'postgres', 'migrate', 'grant', 'server', 'web', 'cloudflared', 'backup', 'ledger-capture', 'ledger-prune'}
    if not required <= services.keys():
        errors.add('required_services_missing')
    server = services.get('server', {})
    origin = server.get('environment', {}).get('LIAR_PUBLIC_ORIGIN')
    if not public_origin(origin):
        errors.add('public_origin_invalid')
    if services.get('web', {}).get('build', {}).get('args', {}).get('VITE_PUBLIC_SITE_ORIGIN') != origin:
        errors.add('web_origin_mismatch')
    for name, service in services.items():
        if not IMMUTABLE.fullmatch(service.get('image', '')):
            errors.add('immutable_images_required')
        if service.get('ports') or service.get('network_mode') == 'host' or service.get('privileged'):
            errors.add('public_host_exposure')
        if not service.get('read_only') or 'ALL' not in service.get('cap_drop', []):
            errors.add('container_isolation_invalid')
        for key, value in service.get('environment', {}).items():
            if value and re.search(r'(?:PASSWORD|SECRET|TOKEN|_KEY|_KEYS)$', key) and not key.endswith('_FILE'):
                errors.add('plaintext_secret_configuration')
        for reference in service.get('secrets', []):
            key = reference['source'] if isinstance(reference, dict) else reference
            path = config.get('secrets', {}).get(key, {}).get('file')
            if not path or not private_file(path):
                errors.add('secret_files_not_private')
    for name in ['app', 'db']:
        if not config.get('networks', {}).get(name, {}).get('internal'):
            errors.add('internal_network_missing')
    environment = server.get('environment', {})
    for provider in ['GOOGLE', 'APPLE', 'KAKAO', 'NAVER']:
        if not environment.get(f'LIAR_{provider}_CLIENT_ID'):
            errors.add('four_oauth_providers_required')
    for service_name, path in [('backup', '/backups'), ('ledger-capture', '/ledger')]:
        mounts = services.get(service_name, {}).get('volumes', [])
        if not any(m.get('type') == 'bind' and m.get('target') == path for m in mounts if isinstance(m, dict)):
            errors.add('independent_backup_mount_required')
    # Evidence binds operator observations to this exact origin and release, never a generic checkbox.
    release = {name: services.get(name, {}).get('image') for name in ['server', 'web']}
    if not isinstance(evidence, dict) or evidence.get('version') != 1 or evidence.get('origin') != origin or evidence.get('images') != release:
        errors.add('release_evidence_binding_missing')
        evidence = {}
    limits={key:value for key,value in environment.items() if key.startswith(('LIAR_ONLINE_','LIAR_LOBBY_','LIAR_BOARD_','LIAR_BOT_'))}
    if evidence.get('limits')!=limits: errors.add('capacity_evidence_binding_missing')
    checks = evidence.get('checks', {})
    for key in EVIDENCE:
        record = checks.get(key, {})
        try:
            at = datetime.fromisoformat(record.get('performed_at', '').replace('Z', '+00:00'))
            valid = (record.get('status') == 'passed' and at.tzinfo is not None
                     and 0 <= (now-at).total_seconds() <= 7*86400
                     and isinstance(record.get('record'), str) and 0 < len(record['record'].strip()) <= 1000)
        except (ValueError, TypeError):
            valid = False
        if not valid:
            errors.add('operator_check_required:' + key)
    return {'status': 'ready' if not errors else 'blocked', 'checks': sorted(errors)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--env-file', required=True)
    parser.add_argument('--compose', action='append', required=True)
    parser.add_argument('--evidence', required=True)
    args = parser.parse_args()
    try:
        command = ['docker', 'compose', '--env-file', args.env_file]
        for path in args.compose:
            command += ['-f', path]
        for profile in ['tunnel', 'backup', 'ledger-capture', 'ledger-prune']:
            command += ['--profile', profile]
        result = subprocess.run(command + ['config', '--format', 'json'], stdout=subprocess.PIPE,
                                stderr=subprocess.DEVNULL, timeout=30, check=True)
        config = json.loads(result.stdout)
        evidence = json.loads(Path(args.evidence).read_text())
        report = check(config, evidence)
    except (OSError, ValueError, TypeError, KeyError, AttributeError, subprocess.SubprocessError):
        report = {'status': 'blocked', 'checks': ['configuration_or_evidence_unreadable']}
    print(json.dumps(report))
    return 0 if report['status'] == 'ready' else 1


if __name__ == '__main__':
    raise SystemExit(main())
