# 홈서버 컨테이너 배포 절차

[ADR0097](../adr/0097-isolated-container-deployment.md)·[#211 검증](../verification/211-isolated-container-deployment.md). 이 절차는 단일 인스턴스용이며 실제 공개 도메인/계정·OS/미니PC·회선/Tunnel·OAuth/CMP/광고 검수는 상위24/42/147에 남는다. 자동 가입·포트포워딩·이미지 registry push는 수행하지 않는다.

## 검토된 소스와 이미지

Node LTS·Rust stable·최신 안정 NGINX/PostgreSQL18 공식 기반으로 `deploy/Dockerfile`의 server/web target을 빌드한다. build context는 `.dockerignore`의 허용 source만 전달한다. `/review`, `.tmp`, 환경파일/비밀·사용자 파일을 빌드하지 않는다. 신규 source 형식/정적 파일 추가 때 이 목록과 context 검사를 함께 갱신한다.

```sh
# 실제 값은 저장소 밖의 운영자 환경파일에 작성한다. 비밀 내용은 넣지 않는다.
docker compose --env-file /secure/liar/runtime.env -f deploy/compose.yml config --quiet
docker compose --env-file /secure/liar/runtime.env -f deploy/compose.yml build --pull
```

`runtime.env`의 형식은 [예시](../../deploy/.env.example)다. 확정 HTTPS origin을 서버와 web build에 동일하게 사용한다. origin 변경은 웹 재빌드·검수가 필요하다. 실제광고는 publisher ID만으로 활성화되지 않으며 실제CMP/사이트승인/문서격리·CSP/ads.txt는 [광고 설정](auto-ads-configuration.md)과147을 따른다. 기본 CSP는 외부 광고/CMP script를 허용하지 않는다.

검증된 source commit·기반 digest·완성된 server/web image ID/digest와 static manifest를 별도 운영 기록에 남긴다. `:local`은 로컬 빌드 이름이며 운영 교체에서는 검수한 불변 image reference를 `LIAR_SERVER_IMAGE`/`LIAR_WEB_IMAGE`에 지정한다. 기존 DB volume의 메이저 버전은 base tag 변경만으로 올리지 않는다.

## 비밀 파일과 최소 DB 권한

저장소 밖의0700 전용 디렉터리(`LIAR_SECRETS_DIR`)에 다음 파일을 둔다. 채팅/이슈/명령줄/환경파일·build ARG에 값 자체를 넣지 않는다. 서버 entrypoint가 지정 파일을 읽어 고정 자식 실행 환경으로 전달하며 직접 secret 값과 파일의 충돌·빈 파일/16KiB 초과/NUL/누락은 안정 오류로 종료한다.

| 파일 | 용도·읽는 실행 주체 |
|---|---|
| db_admin_password | DB bootstrap 관리자, postgres OS 사용자 |
| db_owner_password | bootstrap와 migration 뒤 권한 부여, postgres OS 사용자 |
| db_serving_password | bootstrap가 생성하는 liar_app 비밀번호, postgres OS 사용자 |
| migration_database_url | `postgres://liar_owner:<인코딩한 owner 암호>@postgres:5432/liar`, migration UID10001 |
| database_url | `postgres://liar_app:<인코딩한 serving 암호>@postgres:5432/liar`, server UID10001 |
| csrf_key |32바이트의 base64url/no-padding, server UID10001 |
| digest_keys, vault_keys | 기존 OAuth 설정의 current/version/key JSON, server UID10001 |
| tunnel_token | 실제 connector 등록 뒤만 사용, cloudflared UID65532 |

실제 암호·키는 운영자가 안전한 로컬 생성/보관 수단으로 준비하며 provider 키를 추정하지 않는다. URI 암호 부분은 URL 인코딩하고 postgres/owner/app 비밀번호를 서로 분리한다. `.env` 치환으로 암호를 주입하지 않는다. secret 파일은 호스트 소유권/0400 또는 필요한 전용 그룹0440으로 제한한다. Compose file secret은 host bind 권한을 따르므로 `uid/mode` 선언만으로 변경됐다고 간주하지 않는다. 사용한 PostgreSQL 이미지의 실제 OS UID를 확인해 bootstrap 파일 권한을 맞추고 UID10001/101/65532의 읽기·쓰기 범위를 실제 컨테이너에서 검수한다. Docker daemon 권한은 이 비밀을 읽을 수 있는 운영 권한이다.

새 전용 `pgdata` volume에서만 `init-db.sh`가 postgres와 별개인 NOSUPERUSER/NOCREATEDB/NOCREATEROLE `liar_owner`/`liar_app`을 만든다. `migrate`만 DDL 자격을 받고 기존 forward migrations를 적용한다. `grant`는 owner 자격으로 [serving-grants.sql](../../deploy/serving-grants.sql)을 적용한다. serve에는 DML URL만 mount하며 CREATE/DDL·migration table 쓰기·raw payload UPDATE를 주지 않는다. 두 one-shot 단계가 성공한 뒤만 server가 시작한다.

이미 초기화한 volume에서는 비밀 파일 교체만으로 DB 암호가 바뀌지 않는다. 기존 volume에 bootstrap를 재실행하거나 강제로 삭제하지 말고 운영자가 별도 암호 회전/DB 역할·schema 호환 절차를 수행한다. `docker compose up`은 검토된 migration/grant를 실행하므로 **업데이트 전에 백업과 복원 검수·이전 image/schema 호환을 먼저 확인한다.**

## 기동·네트워크·교체

```sh
# 기본 profile은 실제 provider/CMP/광고와 Tunnel을 활성화하지 않는다.
docker compose --env-file /secure/liar/runtime.env -f deploy/compose.yml up -d --wait
docker compose --env-file /secure/liar/runtime.env -f deploy/compose.yml ps
# 새 매칭 중지·기존 판/준비 등록/결과 저장 drain 후 정상 종료를 확인한다.
docker compose --env-file /secure/liar/runtime.env -f deploy/compose.yml stop server
```

설정에 host `ports`가 없으며 DB와 app은 internal이다. server 전용 outbound는 OAuth/JWKS/Apple 요청용이며 public inbound를 추가하지 않는다. nginx와 connector만 edge를 공유하고 고정 `web:8080`으로 Tunnel origin을 지정한다. Host/전달IP로 upstream을 만들지 않고 공개 admin은 없다. 유지보수는 호스트의 제한 SSH/VPN에서 별도 DB CLI로 수행한다.

서버 PID1은 Rust이며 `stop_grace_period=320s`로300초 drain·5초HTTP·2초CPU 정리보다 여유를 둔다. `stop -t 10` 등으로 이를 줄이면 저장 완료 전에 강제 종료될 수 있다. drain 중 ready503/live200이며 기존 WS/HTTP 재접속 경로는 유지한다. ready 실패를 이유로 기존 origin을 즉시 제거하는 외부 라우팅 규칙을 쓰지 않는다. 정상 종료 status·저장·journal 정리를 실제로 확인하고 이전 인스턴스가 완전히 종료된 후 새 인스턴스를 시작한다. DB owner를 공유한 동시 두 인스턴스를 기동하지 않는다.

nginx는 기존32locale HTML·동일원천 문서 route·개인 noindex/no-store·실제403/404·API/WS를 제공한다. hash asset만 immutable, worker/HTML은 재검증하고 API/cookie는 캐시하지 않는다. access log는 끄고 구조화 앱/DB 로그는 용량 회전한다. rotation 설정은 크기 제한이며7일 보존 실측을 주장하지 않는다. nginx의3MiB transport 상한은 기존2MiB replay+envelope를 통과시키며 실제 허용 입력은 서버가 판정한다. 요청 대기·임시 공간/용량은 실장비26에서 측정한다.

## 실제 외부 연결

네 제공자 등록을 완료하면 [OAuth 설정](oauth-configuration.md)대로 키/최소 scope/callback·Apple 파일·notification audience를 검수하고 [provider override 예시](../../deploy/providers.example.yml)를 저장소 밖에서 실제 환경과 함께 사용한다. 파일 내용은 경로로만 전달한다. 예시는 실계정 검수나 키를 제공하지 않는다.

Cloudflare 계정/도메인/connector 등록은 운영자가 한다. 실제 token 파일을 준비하고 dashboard의 hostname→`http://web:8080` 경로를 확인한 뒤에만 명시적으로 `--profile tunnel`을 추가한다. 이 작업에서 실제 Tunnel 연결은 하지 않는다. origin IP/게임·DB 포트를 공개하는 우회는 하지 않는다. site/domain/CSP/실광고·동의 거부/철회·8언어/실기기·회선 약관/전원 검수 전 공개 완료로 판단하지 않는다.

## 로컬 자동 검증과 정리

```sh
# Docker 엔진·프로젝트 Node/Yarn·Chromium이 준비된 로컬/CI용이다.
corepack yarn playwright install chromium
python3 deploy/tests/smoke.py --browser
```

검사는 `liar-smoke-<random>` 프로젝트와 `.tmp`의 합성 비밀만 만든다. 처음 기동 상태에서 host published ports0을 검사한다. 실제 Chromium/WS 연결 단계에서만 생성한 override로127.0.0.1의 임의 포트를 열며 운영 compose에는 이 mapping이 없다. no-JS32문서와 실제 Rust WASM 튜토리얼을 검사한 뒤 제한 계정/세션 fixture로 인증 WS·자연 초기연결 취소의 실제 Docker stop/저장을 확인한다. 이는 실제 OAuth 인증/4분 전체 게임·최종 network ACK·운영 장비/실Tunnel 성공의 증거가 아니다.

성공/실패 뒤에도 소유한 프로젝트의 컨테이너·네트워크·볼륨과 합성 비밀만 정리한다. 이미지/검증 summary는 남는다. 운영 DB에는 `down --volumes`를 사용하지 않는다. 백업·암호화·별도매체/복원·롤백 훈련과 측정 RPO/RTO는 #25, 미니PC 부하/성능은 #26의 미완료 출구다.
