# #122 인증 데일리 시간 HTTP

[이슈122](https://github.com/crystal23733/search-mine/issues/122) · M3 · [ADR0057](../adr/0057-authenticated-daily-timing-http.md).

선행 #120/PR121 develop08eede7efb93777a06eb483fda81b60a956bcaa7 병합·#120 CLOSED 2026-10-11T15:51:28Z 확인 후 시작했다. ADR/프로토콜·DB/운영/privacy/testing/design18/TS/추적을 제품 코드 전에 반영했다. domain03과 공개 HTTP 표도 최종 계약으로 정리했다.

## 변경과 실제 리뷰

closed Rust→TS/start-heartbeat-submit-timed를 기존 동일 HTTP context/requests4/rate20초당/전체2초/CPU2·2초에 연결했다. 현재 account/revision/nickname/Origin/CSRF·전후 실제 session proof·revocation select/응답 barrier를 유지한다. DailyAuthentication 생성자가 ticket registry에서 같은 authority Arc를 유도한다. main과 HTTPS fixture가 같은 registry를 실제 invalidator와 HTTP에 주입하며 중복 daily authority child는 없다. 매 요청 reap-before-generation/read/bind, config tickets64/1..20000, auth disabled 경로503/no-store다.

start는 서버 UTC today/seed version1/accepted snapshot을 선택하고 자정 사이 IO는 Conflict로 재시도한다. chrono 최신0.4.45를 버전 지정 없이 cargo add, 필요한 std 기능만 활성화·lock 갱신. heartbeat 연속순번/원래duplicateACK/15초 권장·45초 gap/TTL과 sealed time은 기존 정책을 사용한다. 필수 nullable ticket_id 누락은 invalid, null/만료/재구성 소실/gap/too-fast는 완료만 기록한다. bad attempt/accepted metadata는 오류로 끝나며 fallback하지 않는다. Native 검증/commit은 plain/timed 단일 흐름이다. 저장된 첫 Some/NULL·retry/Practice·본인 export/삭제는 불변이다.

pm-ai-shipping:code-review correctness/changes/baseline08eede7을 연결 범위에서 순차 수행했다. proposed countdown3000 vs accepted5000, 원래9000 vs 새 요청, 같은 attempt 동시 start/finish의 원래 ticket/receipt, 다른 attempt/date/no-write, capacity1 expired last-lease 뒤 다른 account admission, 실제 Native worker live 중 plain/timed logout/erasure를 강제했다. registry 재구성에서 원래 ticket은 NULL 완료이며 OS 재시작 실험으로 계산하지 않는다. typed same-authority 조립/sealed proof/current auth PG/저장 receipt 기반 trust를 보존한다. 검토 범위에서 추가로 근거가 뒷받침된 제품 결함은 없다. 별도 보안·성능 감사/인간/글로벌RTT 검증은 수행하지 않았다.

## Red·Green과 준비 실패 구분

공식 start 실제Red404vs200/0pass1fail/0.00초→HTTP11/5.40초 후 UTC 자정 보강을 최종full에서 검사했다. 실제 새PG3/3.88초 뒤 wrong metadata/attempt/no-fallback·원래 ticket의 registry 재구성·live native 철회/삭제 확장 HTTP PG7/3.49초를 실행했다. 이후 동시 같은 attempt start/finish를 fresh PG96에 포함했다. PC1440/mobile390 actualHTTPS2/41.2초/재시도0(각6.3/5.3초)은 actualWASM·주입fixtureclock heartbeat·Some 저장/NULLticketretry·8locale 다운로드·로그아웃 stale401을 확인했다. 주입 clock을 인간 풀이/벽시계/사전 계산 방지/RTT 성적으로 주장하지 않는다.

첫 migration preflight build는 실제101/OS112 디스크 공간 부족, 다음 migration은 실제1 checksum 불일치였고 full은 두 경우 모두 시작 전이었다. 실제psql public 최신202610120001·time열1, SHA384 working/Git/LF166bytes14de77… vs DBstored f55982…는 CRLF169bytes와 정확일치했다. #120 미커밋CRLF 적용 후 Git LF checkout의 로컬 준비 이력 차이다. 원본 DB/checksum/Git migration을 변경하지 않고 별도 loopback liar_daily122 DB에 Git migration을 적용했다(실제0/latest checksum14de77… 확인). 완료112/114/120 generated coverage target만 exact workspace/nonreparse/로그·JSON 보존 확인 후 정리했다. 별도의 제품 회귀 실패로 계산하지 않는다.

## 최종 로컬 검증

재개 전 계측 빌드는 rustc 프로세스 종료로 실패해 JSON이 없었고 원인을 확정하지 않았다. 실패 로그를 보존하고 같은 격리 target에 실행 profraw가 0개임을 확인한 뒤 빌드 jobs2로 재개했다. 재개 직후 일반 PG 회귀는 loopback54570 응답 없음 상태에서 실제23pass/73fail/118.95초/exit101이었다. 기존 .tmp/pg70 데이터는 초기화하지 않고 숨김 pg_ctl로 다시 기동해 isready/17.4/liar_daily122를 확인했다. 이후 계측 전체와 일반 PG 전체를 각각 통과했다. 환경 준비 실패와 최종 성공을 구분하며 빌드 종료의 근본 원인을 단정하지 않는다.

최종 scripts/check.ps1 실제Exit0: Rust387·web221/38files/Lines88.85%·browser102/3.8m/재시도0·fmt/Clippy/WASM/types/fixtures/tokens/format/lint/typecheck/build·문서172/추적36/Mermaid34실패0. actualPG96/13.89초 및 격리 instrumentation PG96/16.42초: Daily782/798 = 97.99%·Privacy164/164 = 100.00%·서버8820/9422 = 93.61%, domain95/전체80/entry point 기준 유지. 측정 후 Rust source/test 변경 없음.

원격 PR/CI·exact-head 병합/122CLOSED는 다음 확인 단계다. 두 순위/명시8언어 공식 시작UI/owned pending·상위19·외부42·물리25·하드웨어26·사람27·미확정SW83은 미완료다.


## 병합·종료 확인

PR123은 2026-10-12T11:25:23Z에 develop1414cda22708b13af1207f98215069b89edc2381로 병합됐고 #122 CLOSED 11:25:24Z를 별도 확인했다. 최신8checks/3runs와 정확b9349dc·clean tree를 확인했다. CI PG18.6 회귀96/33.11초·계측96/34.02초, Daily97.99%/Privacy100%/서버93.61%, 웹221/38files/line88.85%, Chromium102/4.6분·재시도 관측0. 다음 두 순위124와 공식 웹/상위19는 미완료다.
