"""Check measured auth domain and whole server line coverage, including entry points."""
import json
from pathlib import Path
import sys

data = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
server = [f for f in data["data"][0]["files"] if "/crates/server/src/" in f["filename"].replace("\\", "/")]
domain = [f for f in server if f["filename"].replace("\\", "/").endswith(("/auth/model.rs", "/auth/service.rs"))]
match_domain = [f for f in server if f["filename"].replace("\\", "/").endswith("/online/state.rs")]
if len(domain) != 2 or len(match_domain) != 1 or not any(f["filename"].endswith("main.rs") for f in server) or not any(f["filename"].endswith("migrate.rs") for f in server):
    raise SystemExit("Coverage is missing domain or executable entry points")
failed = False
for name, files, minimum in [("Auth domain", domain, 95), ("Match domain", match_domain, 95), ("Whole server", server, 80)]:
    count = sum(f["summary"]["lines"]["count"] for f in files)
    covered = sum(f["summary"]["lines"]["covered"] for f in files)
    percent = covered * 100 / count if count else 0
    print(f"{name}: {covered}/{count} lines = {percent:.2f}% (required {minimum}%)")
    failed |= percent < minimum
raise SystemExit(1 if failed else 0)
