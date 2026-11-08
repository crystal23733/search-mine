# ADR0097 · 외부 입력 없는 격리 컨테이너 배포 기반

- 상태: 채택 · 승인된 FR01/10/11/14/16·NFR01/02/05/07/08 구체화
- 선행 #208 CLOSED/PR210 MERGED · 하위 #211/M5 · 상위 #24의 실제 계정/호스트 검수 유지
- 추적 TS15/16/18/19/20/21/26/28/31/35/36

## 이미지·비밀·권한

build context는 저장소 전체 허용 대신 필요한 source/manifests/scripts/deploy만 허용한다. .git/.tmp/review/환경파일·secret/사용자파일/target/node_modules와 중첩 환경파일도 제외한다. Rust stable·Node LTS·NGINX stable/PostgreSQL18 공식 이미지를 실제 조회/빌드해 호환성을 검증하며 lockfile immutable/locked와 기존 wasm 선언 검사를 유지한다. base tag는 운영 build 시 갱신 가능하고 실제 배포에는 검증한 image digest를 기록한다. 비밀값은 ARG/빌드환경/레이어에 넣지 않는다. 웹의 확정 public origin/게시자ID만 공개 build 설정이다. 미설정 광고/CMP 기본 차단은 유지한다.

serve·migration은 별도 명령/파일 secret/역할로 실행한다. entrypoint는 고정 허용한 바이너리만 exec하고 알려진 _FILE 변수만 파일에서 읽어 자식 환경으로 전달한다. 임의 shell/eval/source·값/경로 로그·secret argv는 없다. 누락/비정상/직접값과파일 충돌은 안정 오류로 종료한다. Apple private key는 기존 파일 port를 사용한다. 실제 provider 키는 profile/운영자 설정으로 추가하며 기본 실행에서 설정된 척하지 않는다.

서버 UID10001·nginx 비권한UID·read-only rootfs·cap_drop/no-new-privileges·제한 tmpfs를 사용한다. PG 데이터 전용 볼륨과 DDL/DML 자격을 분리한다. 최소 serving 권한은 기존 migration/table·유지보수 SELECT/INSERT/DELETE와 허용 UPDATE·sequence만 부여하고 소유자/CREATE/DDL을 주지 않는다. secret의 파일 mount UID/mode는 host 원본 권한을 따르는 Compose 동작을 고려해 실제 컨테이너에서 읽기 검수한다. 실제 키/로그인/최소scope 검수는 #42다.

## 네트워크·종료

edge: cloudflared/nginx, app internal: nginx/server, db internal: server/postgres/migration, host published ports0. 서버는 OAuth/JWKS/Apple 철회를 위해 명시 outbound network에도 연결한다. 기존 internal app+db만 연결하면 외부 인증 요청이 불가능하므로 이를 보완하되 외부 inbound publish는 추가하지 않는다. migration과postgres는 outbound network에 연결하지 않는다. cloudflared는 실제tokenfile/domain 확보 후 명시 profile로만 실행하며 자동 가입/실Tunnel연결을 하지 않는다.

serve의 PID1은 실제 Rust binary이며 stop_grace_period는 기존300초 drain+5초HTTP+2초runtime보다 여유 있게 둔다. health는 내부 live/ready를 사용하고 drain 중 ready503 때문에 기존 WS 재접속 경로를 강제 제거하지 않는다. 실제 로컬 Docker stop 검증은 호스트·실4분 전체운영검수와 구분한다. 실제 단일 인스턴스 교체 전에 백업/이전image·호환schema를 확인하며 무중단 다중 인스턴스를 주장하지 않는다.

## nginx 문서·proxy

빌드된32개 공개 locale HTML만 query 없는 정확 경로에서 제공한다. 그 외 앱 경로/query는 noindex/no-store generic shell을 사용하고 403/404는 실제 HTTP 상태를 유지한다. 명시 고정 경로 allowlist로 임의 파일·dotfile·환경/원문/secret을 제공하지 않는다. API·WS는 고정 server upstream이며 요청 Host/전달IP로 upstream을 만들지 않는다. private/cookie/API 캐시는 차단하고 hash asset만 immutable, service-worker/HTML은 재검증한다. 요청 query/IP/cookie/body/사용자값 access log를 남기지 않는다. 필수 nosniff/referrer/frame/CSP는 실제 OAuth redirect/WS·WASM·worker를 허용하되 실제 광고/CMP 운영CSP는 #147에서 별도 검수한다.

## 검증·외부 출구

구성 부재 Red→실제 source build/run Green, 격리된 임시 secret/네트워크/볼륨에서 migration·DML 권한·ready/live/disabled 제공자·32locale/JSoff·query/noindex/상태/캐시·API/WS·read-only/nonroot/공개포트0/정상stop·비밀/사용자파일 context 제외를 검증한다. 자동 검증과 operator runbook을 남긴다. 실제 도메인/계정/Tunnel·미니PC/전원/회선·실광고/CMP·별도매체백업/RPO/RTO/부하는 미완료이며 상위24~26/42/147/83을 유지한다.

근거: [Docker Compose secrets](https://docs.docker.com/compose/how-tos/use-secrets), [services/stop_grace_period](https://docs.docker.com/reference/compose-file/services/), [공식 Rust 태그](https://github.com/docker-library/official-images/blob/master/library/rust), [공식 Node 태그](https://github.com/docker-library/official-images/blob/master/library/node), [NGINX 비권한 이미지](https://github.com/nginx/docker-nginx-unprivileged).
