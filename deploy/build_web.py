"""Build a web image with validated public settings; never upload operator files as context."""
import argparse
import hashlib
from pathlib import Path
import subprocess


def command(origin, tag, config=None, publisher=None):
    result = ["docker", "build", "--target", "web", "-f", "deploy/Dockerfile", "-t", tag,
              "--build-arg", "VITE_PUBLIC_SITE_ORIGIN=" + origin]
    if config:
        path = Path(config).resolve(strict=True)
        raw = path.read_bytes()
        if not raw or len(raw) > 512 * 1024:
            raise ValueError("Public configuration must be 1..512 KiB")
        result += ["--build-arg", "LIAR_PUBLIC_CONFIG_SHA256=" + hashlib.sha256(raw).hexdigest(),
                   "--secret", "id=public_site_config,src=" + str(path)]
    if publisher:
        if not config:
            raise ValueError("Google mode requires published policies and CSP origins")
        result += ["--build-arg", "VITE_CMP_PROVIDER=google", "--build-arg", "VITE_ADSENSE_CLIENT_ID=" + publisher]
    return result + ["."]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--origin", required=True)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--config")
    parser.add_argument("--publisher")
    args = parser.parse_args()
    raise SystemExit(subprocess.run(command(args.origin, args.tag, args.config, args.publisher), cwd=Path(__file__).resolve().parents[1]).returncode)


if __name__ == "__main__":
    main()
