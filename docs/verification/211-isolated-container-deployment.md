# #211 · 격리 컨테이너 배포 기반

[ADR0097](../adr/0097-isolated-container-deployment.md) · FR01/10/11/14/16·NFR01/02/05/07/08·TS15/16/18/19/20/21/26/28/31/35/36.

선행208CLOSED/PR210MERGED·최신developfb7995a clean/원격0:0·openPR0을 확인했다. 제품 전 설계/시나리오/추적을 기록했다. 실제 Red/Green·검증·리뷰·PR/병합은 실행 후 기록한다. 실제 계정/도메인/Tunnel/호스트·실Windows/부하·별도매체RPO/RTO·83은 별도 미완료다.

## 실제 Red/Green와 실행

- 실제 Red: 배포 target/source build가 `lstat deploy: no such file or directory`로 실패했다(`.tmp/deploy211-red.log`). Docker 엔진이 꺼진 것을 확인한 뒤 로컬 엔진만 시작했다.
- 실제 source server/web build PASS: Rust1.99.1·Node LTS26.13.0·wasm-bindgen CLI0.2.130(기존 lock0.2.130과 일치), NGINX stable1.30.6/PostgreSQL18.6. 의존성/lockfile이나 선언을 구버전 pin/우회하지 않았다. source context의 root·web/Rust source 안 가짜.env·.git/.tmp/review/.aws 제외를 실제 builder 컨테이너에서 확인했다.
- 고유 프로젝트/새 전용 volume·합성 파일만 사용한 최종 `python3 deploy/tests/smoke.py --browser` 전체 PASS(`.tmp/deploy211-smoke-final.log`, `.tmp/liar-smoke-8c2de77184-olath3qm/summary.json`). server image26683da72bdf…/webc724b3f47bae…를 실제 실행했다. 기본 production config와 기동 시 host published port0·실 Docker app/db internal·정확 network attachments·비권한/read-only/PID1·쓰기 차단을 확인했다.
- 별도 owner migration/grant 성공·제한 serving 자격으로 ready200/metric 초기화, CREATE TABLE 실패·raw payload UPDATE/migration table 쓰기 금지·최소 UPDATE(id)만 허용을 검사했다. 누락/직접값 충돌/빈·16KiB 초과/NUL 파일/허용하지 않은 명령은 exit78·안정 메시지이며 값/임의 실행을 노출하지 않았다.
- nginx32 공개 HTML의8언어/h1/canonical·9alternate/sitemap32, 개인/query generic noindex/no-store·실403/404·dotfile/임의HTML차단, hash immutable·누락 asset no-store·worker no-cache·CSP/nosniff/referrer/frame·health/API를 실제 HTTP로 검사했다. 기존2MiB replay payload를 프록시가 잘라413으로 만들지 않는 것도 확인했다.
- 처음 port0 검증 뒤 test override에서만127.0.0.1 임의 port로 실제 Chromium 모바일 JSoff32문서·동일canonical/9alternate·실제 Rust WASM9칸 튜토리얼·개인 head noindex를 검사했다. CSP를 우회하지 않았다.
- 제한 DB의 합성 계정/세션으로 실제 auth/CSRF·queue→본인 match와 nginx를 통한 WS101·첫 공개 snapshot을 즉시 관측했다. active journal1인 상태에서 실제 Docker stop을 보낸 뒤3.050초에 exit0/OOMfalse·원래 cancelled 결과·active0을 확인했다. 다른 참가자가 붙지 않은 자연 초기 취소이며 실제4분 전체 판·최종network ACK의 증거는 아니다.
- cloudflared는 network none에서 `tunnel run --help`의 실제 --token-file 옵션만 검사했다. 실제 token/connector/Tunnel 연결·계정 가입·도메인/운영자 설정·이미지 push는 하지 않았다.

## 실행 중 수정·리뷰

첫 기동의 tmpfs YAML flow-list 따옴표와 psql PGDATABASE 환경의 URI 해석을 실제 실패로 확인했다. grant는 고정 DB 필드+파일 PGPASSWORD 환경으로 바꾸어 비밀 argv를 만들지 않았다. 리뷰에서 nginx24KiB 임시 상한이 기존2MiB daily 계약을 자름을 찾아3MiB transport ceiling으로 고쳤으며 실제 판정은 기존 Rust guard다. 오류 status의 no-store/noindex와 missing asset negative cache도 검사했다.

검증 fixture는 기존 네 제공자 available=false 계약·초 단위 세션 시각과 맞췄다. curl 완료 기한을 기다리다 초기 취소가 끝나는 관측 오류는 첫 WS frame을 즉시 읽는 이벤트 관측으로 수정했고 기존 browser/native timeout/retry/flags는 바꾸지 않았다. 로컬 실패 원문과 성공을 `.tmp/deploy211-*`에 구분했다. 합성 비밀·생성.env probe·소유한 project/container/network/volume만 정리하고 이미지/summary는 유지한다. 실패 중 생성한 invalid-secret도 정리하며 실제 label별 잔여0 확인과 단독 cleanup 검사도 통과했다.

실제 코드/diff 리뷰: 고정 allowlist source/exec·파일 secret 권한·DDL/DML mount 분리·원자 권한 transaction·공개파일 exact map/request query privacy·고정 upstream/전달IP 무신뢰·32locale 원천 재사용·320초 stop과 PID1·시험 resource 소유 범위를 확인했다. 서버 도메인/프로토콜·게임규칙·8언어/광고/동의·개인 다운로드/API·의존성·기존6CI를 변경하지 않았다. 새 deployment job은 이 실제 검사를 추가한다. local Node/Python syntax·문서257/Mermaid34/diff PASS. 로컬 전체 Rust/web/LLVM 기존 검사는 이번 배포 변경에서 재실행하지 않았으며 원격7CI로 확인한다.

실제계정/도메인/Cloudflare·CMP/실광고·Windows/미니PC/회선·실4분/부하·별도매체backup/RPO/RTO는 미실행·후속이다. 상위24~26/42/147/83을 완료 처리하지 않는다. CI/정확head·리뷰/병합은 실행 뒤 PR/로컬 review에 기록한다.
