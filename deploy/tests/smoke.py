"""Exercise only a fresh, owned Compose project. No operator settings or accounts are used."""
import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import re
import secrets
import shutil
import socket
import subprocess
import tempfile
import time
import uuid

ROOT = Path(__file__).resolve().parents[2]
ORIGIN = "https://sweeper.example.test"
LOCALES = ["en", "ko", "ja", "zh-CN", "es", "pt-BR", "de", "fr"]


def require(condition, message):
    if not condition:
        raise AssertionError(message)


class Smoke:
    def __init__(self, browser):
        self.project = "liar-smoke-" + uuid.uuid4().hex[:10]
        self.folder = Path(tempfile.mkdtemp(prefix=self.project + "-", dir=ROOT / ".tmp"))
        self.secret_dir = self.folder / "secrets"
        self.secret_dir.mkdir(mode=0o700)
        self.values = {name: secrets.token_urlsafe(32) for name in ["db_admin_password", "db_owner_password", "db_serving_password"]}
        self.values["migration_database_url"] = f"postgres://liar_owner:{self.values['db_owner_password']}@postgres:5432/liar"
        self.values["database_url"] = f"postgres://liar_app:{self.values['db_serving_password']}@postgres:5432/liar"
        key = lambda: base64.urlsafe_b64encode(secrets.token_bytes(32)).decode().rstrip("=")
        self.values["csrf_key"] = key()
        for name in ["digest_keys", "vault_keys"]:
            self.values[name] = json.dumps({"current": 1, "keys": [{"version": 1, "key": key()}]})
        for name, value in self.values.items():
            p = self.secret_dir / name
            p.write_text(value)
            # Synthetic files only. The 0700 parent is inaccessible to other host users.
            p.chmod(0o444)
        self.server_image = "liar-server:" + self.project
        self.web_image = "liar-web:" + self.project
        self.env = self.folder / "compose.env"
        self.env.write_text(f"LIAR_PUBLIC_ORIGIN={ORIGIN}\nLIAR_SECRETS_DIR={self.secret_dir}\nLIAR_SERVER_IMAGE={self.server_image}\nLIAR_WEB_IMAGE={self.web_image}\n")
        self.env.chmod(0o600)
        self.compose = ["docker", "compose", "--env-file", str(self.env), "-p", self.project, "-f", "deploy/compose.yml"]
        self.browser = browser
        self.results = {}
        self.markers = []

    def run(self, args, data=None, okay=True, timeout=180):
        if args[:2] == ["docker", "build"]:
            log = self.folder / ("build-" + args[args.index("--target") + 1] + ".log")
            with log.open("wb") as stream:
                result = subprocess.run(args, input=data, stdout=stream, stderr=subprocess.STDOUT, cwd=ROOT, timeout=timeout)
            result.stdout, result.stderr = log.read_bytes(), b""
        else:
            result = subprocess.run(args, input=data, stdout=subprocess.PIPE, stderr=subprocess.PIPE, cwd=ROOT, timeout=timeout)
        if okay and result.returncode:
            # Process diagnostics may include fixture paths. Never emit secret values or responses.
            text = (result.stdout + result.stderr).decode(errors="replace")
            for value in self.values.values():
                text = text.replace(value, "[fixture redacted]")
            with (self.folder / "failure.log").open("a") as stream:
                stream.write(text)
            raise AssertionError(f"Owned fixture command failed: {args[0]} (exit {result.returncode}); stage diagnostics saved")
        return result

    def compose_run(self, *args, **kwargs):
        return self.run(self.compose + list(args), **kwargs)

    def stage(self, name):
        print(name, flush=True)

    def cid(self, service):
        return self.compose_run("ps", "--all", "--quiet", service).stdout.decode().strip()

    def inspect(self, service):
        return json.loads(self.run(["docker", "inspect", self.cid(service)]).stdout)[0]

    def request(self, path, method="GET", headers=None, body=None):
        lines = ["silent", "show-error", 'noproxy = "*"', 'max-time = "5"', "include", "path-as-is"]
        lines += ["url = " + json.dumps("http://web:8080" + path), "request = " + json.dumps(method)]
        for key, value in (headers or {}).items():
            lines.append("header = " + json.dumps(f"{key}: {value}"))
        if body is not None:
            lines.append("data-binary = " + json.dumps(body))
        result = self.compose_run("exec", "-T", "server", "curl", "--config", "-", data=("\n".join(lines) + "\n").encode(), okay=False)
        require(result.returncode == 0, "Internal HTTP request failed")
        header, payload = result.stdout.split(b"\r\n\r\n", 1)
        lines = header.decode().split("\r\n")
        status = int(lines[0].split()[1])
        fields = {}
        for line in lines[1:]:
            key, value = line.split(":", 1)
            fields.setdefault(key.lower(), []).append(value.strip())
        return status, fields, payload

    def sql(self, query, app=False, okay=True):
        password = "db_serving_password" if app else "db_admin_password"
        user = "liar_app" if app else "postgres"
        # Only fixed names enter this shell; neither query nor credentials enter argv.
        script = f'PGPASSWORD=$(cat /run/secrets/{password}); export PGPASSWORD; exec psql --no-psqlrc -h 127.0.0.1 -U {user} -d liar -At -v ON_ERROR_STOP=1'
        return self.compose_run("exec", "-T", "postgres", "sh", "-c", script, data=query.encode(), okay=okay)

    def build(self):
        self.stage("Building restricted source images")
        suffix = uuid.uuid4().hex
        for parent in [ROOT, ROOT / "apps/web/src", ROOT / "crates/server/src"]:
            marker = parent / (".env.deploy-probe-" + suffix)
            require(not marker.exists(), "Probe must never replace a user file")
            marker.write_text("excluded-context-fixture")
            self.markers.append(marker)
        source_image = "liar-source:" + self.project
        for target, tag in [("source", source_image), ("server", self.server_image), ("web", self.web_image)]:
            self.run(["docker", "build", "--target", target, "-f", "deploy/Dockerfile", "-t", tag, "--build-arg", "VITE_PUBLIC_SITE_ORIGIN=" + ORIGIN, "."], timeout=1200)
        checks = ["test -f /work/crates/core/src/lib.rs", "test ! -e /work/.git", "test ! -e /work/.tmp", "test ! -e /work/review", "test ! -e /work/.aws"]
        checks += ["test ! -e /work/" + str(p.relative_to(ROOT)) for p in self.markers]
        self.run(["docker", "run", "--rm", "--network", "none", "--entrypoint", "sh", source_image, "-c", " && ".join(checks)])
        self.results["excluded_context"] = True
        for image in [self.server_image, self.web_image]:
            info = json.loads(self.run(["docker", "image", "inspect", image]).stdout)[0]
            serialized = json.dumps(info)
            require(not any(value in serialized for value in self.values.values()), "Fixture secret appeared in image config")
            self.results[image.split(":")[0]] = info["Id"]

    def secret_failures(self):
        self.stage("Checking fail-closed file secret and fixed exec boundaries")
        fixture = self.folder / "invalid-secret"
        tests = [([], ["serve"]), ([], ["/bin/sh", "-c", "printf unexpected"])]
        for value in [b"", b"x" * 16385, self.values["database_url"].encode() + b"\x00"]:
            if fixture.exists():
                fixture.chmod(0o600)
            fixture.write_bytes(value)
            fixture.chmod(0o444)
            flags = ["--mount", f"type=bind,src={fixture},dst=/run/fixture,readonly", "--env", "DATABASE_URL_FILE=/run/fixture"]
            result = self.run(["docker", "run", "--rm", "--network", "none", "--read-only", "--cap-drop", "ALL", *flags, self.server_image, "metrics", "read", "weekly_nsm", "2026-11-02"], okay=False)
            require(result.returncode == 78 and result.stdout == b"" and result.stderr.strip() == b"Invalid runtime secret configuration", "Invalid secret reached the binary or exposed its value")
        tests.append((["--env", "DATABASE_URL=fixture-plaintext", "--env", "DATABASE_URL_FILE=/run/fixture", "--mount", f"type=bind,src={fixture},dst=/run/fixture,readonly"], ["serve"]))
        for flags, command in tests:
            result = self.run(["docker", "run", "--rm", "--network", "none", "--read-only", "--cap-drop", "ALL", *flags, self.server_image, *command], okay=False)
            require(result.returncode == 78 and result.stdout == b"" and result.stderr.strip() == b"Invalid runtime secret configuration", "Secret conflict or unsupported command was not rejected cleanly")
        fixture.unlink()
        self.results["secret_fail_closed"] = True

    def configuration(self):
        active = json.loads(self.compose_run("config", "--format", "json").stdout)
        require("cloudflared" not in active["services"], "Tunnel started without explicit operator profile")
        config = json.loads(self.compose_run("--profile", "tunnel", "config", "--format", "json").stdout)
        for service in config["services"].values():
            require(not service.get("ports"), "Production configuration must publish no port")
            require(not service.get("privileged", False), "No privileged service")
        require(config["networks"]["app"]["internal"] and config["networks"]["db"]["internal"], "App and DB must be internal")
        require("outbound" in config["services"]["server"]["networks"], "OAuth needs explicit egress")
        require(config["services"]["cloudflared"]["profiles"] == ["tunnel"], "Tunnel must require an explicit profile")
        grace = config["services"]["server"]["stop_grace_period"]
        seconds = sum(float(amount) * {"h": 3600, "m": 60, "s": 1}[unit] for amount, unit in re.findall(r"([0-9.]+)(h|m|s)", grace))
        require(seconds > 307, "Stop must exceed the application drain budgets")
        self.results["configuration"] = True

    def containers(self):
        self.stage("Starting isolated DB, migration, grants, server and nginx")
        self.compose_run("up", "-d", "--wait", "--wait-timeout", "180", timeout=200)
        for service in ["postgres", "server", "web"]:
            info = self.inspect(service)
            require(not info["HostConfig"]["PortBindings"], "No running production port may be published")
            require(info["HostConfig"]["ReadonlyRootfs"], "Runtime root must be read-only")
            require(info["Config"]["User"] not in ["", "0", "root"], "Runtime must not run as root")
        expected_networks = {"postgres": {"db"}, "server": {"app", "db", "outbound"}, "web": {"app", "edge"}}
        for service, networks in expected_networks.items():
            actual = set(self.inspect(service)["NetworkSettings"]["Networks"])
            require(actual == {self.project + "_" + network for network in networks}, "Runtime network attachments differ from the reviewed boundary")
        for network in ["app", "db"]:
            actual = json.loads(self.run(["docker", "network", "inspect", self.project + "_" + network]).stdout)[0]
            require(actual["Internal"], "Docker did not isolate the app/DB network")
        require(self.inspect("migrate")["State"]["ExitCode"] == 0 and self.inspect("grant")["State"]["ExitCode"] == 0, "Migration and grants must complete")
        comm = self.compose_run("exec", "-T", "server", "cat", "/proc/1/comm").stdout.strip()
        require(comm == b"liar-server", "Rust server must receive PID1 signals")
        require(self.compose_run("exec", "-T", "server", "touch", "/app/forbidden", okay=False).returncode != 0, "Read-only runtime allowed a write")
        require(self.sql("CREATE TABLE forbidden_ddl(id integer);", app=True, okay=False).returncode != 0, "Serving role acquired DDL")
        checks = self.sql("SELECT has_table_privilege('liar_app','product_events','UPDATE'),has_column_privilege('liar_app','product_events','id','UPDATE'),has_table_privilege('liar_app','_sqlx_migrations','UPDATE');", app=True).stdout.strip()
        require(checks == b"f|t|f", "Serving grants exceed the reviewed minimum")
        self.results["runtime_isolation_and_grants"] = True

    def documents(self):
        self.stage("Checking 32 public documents, private/error routes, assets and API")
        for locale in LOCALES:
            for route in ["/", "/rules", "/storage", "/learn"]:
                path = "/" + locale + route
                status, headers, body = self.request(path)
                require(status == 200 and b"<h1>" in body, "Public JavaScript-off document unavailable")
                require((f'<html lang="{locale}">').encode() in body, "Static locale mismatch")
                require((f'rel="canonical" href="{ORIGIN}{path}"').encode() in body, "Canonical origin/path mismatch")
                require(headers.get("cache-control") == ["no-cache"], "Public document must revalidate")
        for path, expected in [("/ko/me/account?private=fixture", 200), ("/ko/rules?private=fixture", 200), ("/ko/403", 403), ("/de/404", 404), ("/ko/missing", 404), ("/.env", 404), ("/.git/config", 404), ("/ko/rules.html?private=fixture", 404)]:
            status, headers, body = self.request(path)
            require(status == expected, "Private/error HTTP status mismatch")
            require(headers.get("cache-control") == ["no-store"], "Private/error cache mismatch")
            require(headers.get("x-robots-tag") == ["noindex, noarchive"], "Private/error indexing mismatch")
            require(b'rel="canonical"' not in body and b'<h1>' not in body, "Generic shell leaked public metadata/content")
        status, headers, body = self.request("/ko/rules")
        asset = re.search(rb'(?:src|href)="(/assets/[^" ]+\.js)"', body).group(1).decode()
        require("immutable" in self.request(asset)[1]["cache-control"][0], "Hashed asset cache mismatch")
        require(self.request("/service-worker.js")[1]["cache-control"] == ["no-cache"], "Worker update must revalidate")
        require(self.request("/assets/absent-fixture.js")[1]["cache-control"] == ["no-store"], "Missing asset was cached immutable")
        require(self.request("/sitemap.xml")[2].count(b"<url>") == 32, "Sitemap must contain only the 32 public documents")
        for health in ["live", "ready"]:
            require(self.request("/health/" + health)[0] == 200, "Health failed through nginx")
        require(headers["x-content-type-options"] == ["nosniff"] and headers["x-frame-options"] == ["DENY"], "Security headers absent")
        require("wasm-unsafe-eval" in headers["content-security-policy"][0], "WASM CSP absent")
        status, _, body = self.request("/api/v1/auth/bootstrap")
        providers = json.loads(body)["providers"]
        require(status == 200 and len(providers) == 4 and all(not provider["available"] for provider in providers), "Absent real providers must stay disabled")
        status, _, _ = self.request("/api/v1/daily/submit", "POST", {"Content-Type": "application/json", "Origin": ORIGIN}, " " * (2 * 1024 * 1024))
        require(status != 413, "Proxy truncated the existing 2 MiB replay envelope")
        self.results["public_documents"] = 32
        self.results["private_error_cache_api"] = True

    def loopback_fixture(self):
        if hasattr(self, "socket_port"):
            return
        overlay = self.folder / "browser.yml"
        overlay.write_text('services:\n  web:\n    ports:\n      - target: 8080\n        host_ip: 127.0.0.1\n        published: "0"\n        protocol: tcp\n')
        extended = self.compose + ["-f", str(overlay)]
        self.run(extended + ["up", "-d", "--no-deps", "web"])
        port = self.inspect("web")["NetworkSettings"]["Ports"]["8080/tcp"]
        require(len(port) == 1 and port[0]["HostIp"] == "127.0.0.1", "Test fixture must be loopback only")
        self.socket_port = int(port[0]["HostPort"])

    def browser_check(self):
        if not self.browser:
            return
        self.stage("Checking actual Chromium against a test-only loopback mapping")
        self.loopback_fixture()
        self.run(["node", "deploy/tests/browser.mjs", "http://127.0.0.1:" + str(self.socket_port)], timeout=90)
        self.results["chromium_js_off_and_wasm"] = True

    def user(self, name):
        raw = secrets.token_bytes(32)
        token = base64.urlsafe_b64encode(raw).decode().rstrip("=")
        digest = hashlib.sha256(raw).hexdigest()
        account, session = str(uuid.uuid4()), str(uuid.uuid4())
        self.sql(f"INSERT INTO auth_accounts(id,nickname,created_at,last_seen_at) VALUES ('{account}','{name}',date_trunc('second',now()),date_trunc('second',now())); INSERT INTO auth_sessions(id,account_id,token_hash,created_at,authenticated_at,expires_at) VALUES ('{session}','{account}',decode('{digest}','hex'),date_trunc('second',now()),date_trunc('second',now()),date_trunc('second',now())+interval '1 day');")
        cookie = "__Host-liar_session=" + token
        status, headers, body = self.request("/api/v1/auth/bootstrap", headers={"Cookie": cookie})
        require(status == 200 and (json.loads(body).get("account") or {}).get("id") == account, "Synthetic session bootstrap failed")
        browser = next(c.split(";", 1)[0] for c in headers["set-cookie"] if c.startswith("__Host-liar_browser="))
        return {"Cookie": cookie + "; " + browser, "Origin": ORIGIN, "X-Liar-Csrf": json.loads(body)["csrf"], "Content-Type": "application/json"}

    def lobby(self, user, command):
        status, _, body = self.request("/api/v1/lobby", "POST", user, json.dumps({"v": 1, "command": command}))
        if status != 200:
            code = json.loads(body).get("error", "unclassified")
            safe = code if isinstance(code, str) and re.fullmatch(r"[a-z_]{1,40}", code) else "unclassified"
            raise AssertionError(f"Synthetic lobby command failed: HTTP {status}, code {safe}")
        return json.loads(body)

    def websocket_and_stop(self):
        self.stage("Checking authenticated WS proxy and real Docker SIGTERM storage drain")
        self.loopback_fixture()
        users = [self.user("SmokeOne"), self.user("SmokeTwo")]
        for user in users:
            self.lobby(user, {"type": "queue_join", "difficulty": "normal"})
        deadline = time.monotonic() + 5
        match = None
        while time.monotonic() < deadline:
            state = self.lobby(users[0], {"type": "status"})["state"]
            if state["type"] == "matched":
                match = state["match_id"]
                break
            time.sleep(0.1)
        require(match is not None, "Real core preparation did not produce a match")
        # Read the first actual frame immediately; a timed curl capture can consume
        # the initial-connection deadline before the stop is even requested.
        with socket.create_connection(("127.0.0.1", self.socket_port), timeout=3) as connection:
            fields = {
                "Host": f"127.0.0.1:{self.socket_port}", "Cookie": users[0]["Cookie"],
                "Origin": ORIGIN, "Connection": "Upgrade", "Upgrade": "websocket",
                "Sec-WebSocket-Version": "13", "Sec-WebSocket-Key": base64.b64encode(secrets.token_bytes(16)).decode(),
            }
            request = "GET /api/v1/ws HTTP/1.1\r\n" + "".join(f"{key}: {value}\r\n" for key, value in fields.items()) + "\r\n"
            connection.sendall(request.encode())
            pending = bytearray()
            while b"\r\n\r\n" not in pending:
                chunk = connection.recv(4096)
                require(chunk and len(pending) < 16384, "WS handshake did not finish")
                pending.extend(chunk)
            header, remaining = bytes(pending).split(b"\r\n\r\n", 1)
            require(header.split(b"\r\n", 1)[0].split()[1] == b"101", "Authenticated nginx WS upgrade failed")
            pending = bytearray(remaining)

            def take(count):
                while len(pending) < count:
                    chunk = connection.recv(min(65536, count - len(pending)))
                    require(chunk, "WS snapshot ended early")
                    pending.extend(chunk)
                value = bytes(pending[:count])
                del pending[:count]
                return value

            first = take(2)
            require(first[0] == 0x81 and not first[1] & 128, "Expected an unmasked text snapshot")
            length = first[1] & 127
            if length == 126:
                length = int.from_bytes(take(2), "big")
            elif length == 127:
                length = int.from_bytes(take(8), "big")
            require(length <= 1024 * 1024, "Snapshot exceeded the bounded fixture reader")
            snapshot = json.loads(take(length))
            require(snapshot["match_id"] == match and snapshot["payload"]["type"] == "snapshot", "WS snapshot mismatch")
            require(self.sql(f"SELECT count(*) FROM online_active_matches WHERE id='{match}';").stdout.strip() == b"1", "Stop requires a durably active match")
            started = time.monotonic()
            self.compose_run("stop", "server", timeout=40)
            elapsed = time.monotonic() - started
            info = self.inspect("server")
            require(info["State"]["ExitCode"] == 0 and not info["State"]["OOMKilled"], "Docker used a fatal or forced stop")
        reason = self.sql(f"SELECT reason FROM online_match_results WHERE id='{match}';").stdout.strip()
        require(reason == b"cancelled", "Natural initial-connection cancellation was not saved")
        require(self.sql(f"SELECT count(*) FROM online_active_matches WHERE id='{match}';").stdout.strip() == b"0", "Active journal remains after a normal stop")
        self.results["authenticated_ws_and_stop"] = True
        self.results["owned_stop_seconds"] = round(elapsed, 3)

    def cleanup(self):
        self.compose_run("kill", "--signal", "SIGKILL", okay=False, timeout=30)
        self.compose_run("down", "--timeout", "5", "--volumes", "--remove-orphans", okay=False, timeout=45)
        for marker in self.markers:
            marker.unlink(missing_ok=True)
        shutil.rmtree(self.secret_dir)
        (self.folder / "invalid-secret").unlink(missing_ok=True)
        self.env.unlink(missing_ok=True)
        remaining = []
        for kind, flag in [("container", "-a"), ("network", None), ("volume", None)]:
            command = ["docker", kind, "ls", "--quiet", "--filter", "label=com.docker.compose.project=" + self.project]
            if flag:
                command.append(flag)
            result = self.run(command, okay=False)
            if result.returncode or result.stdout.strip():
                remaining.append(kind)
        self.results["cleanup_verified"] = not remaining
        (self.folder / "summary.json").write_text(json.dumps(self.results, indent=2) + "\n")
        require(not remaining, "Owned fixture cleanup is incomplete; inspect its unique project label")
        print("Owned fixture cleanup verified; summary: " + str((self.folder / "summary.json").relative_to(ROOT)), flush=True)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--browser", action="store_true")
    args = parser.parse_args()
    (ROOT / ".tmp").mkdir(exist_ok=True)
    smoke = Smoke(args.browser)
    try:
        smoke.build()
        smoke.secret_failures()
        smoke.configuration()
        smoke.containers()
        smoke.documents()
        smoke.browser_check()
        smoke.websocket_and_stop()
        smoke.results["status"] = "passed"
        print("Deployment smoke PASS", flush=True)
    finally:
        smoke.cleanup()


if __name__ == "__main__":
    main()
