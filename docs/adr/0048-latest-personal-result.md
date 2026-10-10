# ADR0048 · ID 없는 최신 본인 결과 발견

- 상태: 채택 · 2026-10-11 · #105 · 선행103/PR104 develop9eeb36c 병합·종료
- 추적: FR01/11/14/16 · NFR01/02/05/06/07/08 · TS15/16/20/21/26/31/35/36

## 문제

서버 재시작은 active를 복구하지만 새 browser/page는 메모리 match ID를 잃는다. 일반 storage/URL에 온라인 ID를 저장하는 대신 서버가 인증된 본인 결과를 발견하게 한다. 상대/seed/새 개인정보 필드를 추가하지 않는다.

## 서버 계약

POST /api/v1/results/latest의 닫힌 입력은 `{v:1}`뿐이다. account/range/limit/cursor/match ID는 받지 않는다. 응답은 `{v:1,result:PersonalResult|null}`이고 실제 없는 경우만200/null이다. DB/timeout/철회 오류를 null로 바꾸지 않는다. 기존 ID 조회/404·known/unknown 세부 null 계약은 보존한다. Rust 원천에서 TS를 생성한다.

두 결과 API는 같은 Context/Origin·session-bound CSRF·nickname·전체2초/request Semaphore/session·account rate/철회 권위를 공유한다. initial proof→subject-bound read→post proof/same_session→authority guard를 적용한다. latest는 선택한 canonical nonnil row ID와 본인 account를 최소 projection에서 검증한다. disabled 실행도503/no-store이며 query/중복·unknown/body/header 거절을 유지한다.

latest DB 순서는 COALESCE(retention_started_at,recorded_at) DESC, recorded_at DESC, UUID DESC다. 오래된 판의 늦은 복구가 최근 플레이보다 위에 오지 않는다. whole-second tie는 이 안정 정렬로 결정한다. 90일은 정확7776000 elapsed seconds/open boundary이며 anchor>=0/<=now·recorded<=now를 유지한다. 같은 SQL snapshot에서 본인 human participant 최소 칼럼만 조회하고 LIMIT2로 선택한 첫 ID의 participant 중복을 검출하여 실패한다. 두 번째 다른 ID는 반환하지 않는다. 최신 오염 행을 건너뛰어 과거 결과로 성공하지 않는다. 삭제/봇/타인·만료/미래 시각은 기존 정책대로 제외한다. 고정1개 응답이 DB CPU 상수시간을 뜻하지 않으므로 SQL plan/기존 인덱스·deadline·동시 상한을 확인하고 미니PC26 목표 달성을 주장하지 않는다.

## 웹과 디자인

홈의 인증된 사용자에게 최근 결과 링크를 한 번 제공하고 고정 `/{locale}/results`로 이동한다. 주소에 ID/계정/검색 조건을 넣지 않는다. 결과 화면 진입은 명시 조회 의도이며 현재 ready account의 fresh proof로 최신1개를 조회한다. 기존 PersonalResult/공통 Card·Button·NavLink·토큰을 사용하고 다시 대전/홈 동작을 유지한다. 제공04/16홈·10결과·14/15오류 원본을 보존한다.8언어·390/1440px·키보드/aria 상태를 검사한다.

latest port/controller는 요청 owner/account/session·generation/AbortSignal을 메모리에만 보유한다. fetch/body bounded decode는 AuthenticatedRequests.execute 안에서 완료하여 전후 fresh proof를 유지한다. logout/account switch/erasure/offline/권한 없는 recovery candidate는 취소/즉시 숨김이며 늦은 응답을 복원하지 않는다. pending/없음/error/unknown abort를 구분하고 retry를 제공한다. 발견 결과를 현재 live match/승패/자동재입장의 근거로 사용하지 않는다. 비로그인 결과 화면은 로그인 안내만 제공하고 서버 계정을 만들지 않는다. OAuth return_path 범위는 늘리지 않는다.

## 실제 fresh browser 검증

기존 loopback HTTP fixture/PgJournalRuntime+HTTPS 고정 proxy/공개 시험 TLS pin을 유지한다. test-only supervisor가 example binary를 직접 spawn하고 OS kill/exit/다른PID 재시작/ready를 관측한다. 임의 명령/path/upstream을 받지 않고 loopback control에만 둔다. actual product journal/recovery를 사용하며 종료 API로 정상종료를 흉내내지 않는다.

시험 wall-clock은 실제 시스템UTC처럼 재시작에도 현재 값을 유지한다. test-only clock read→fixture전용env 주입만 사용하며 seed/board/UUID/actor state를 복사하지 않는다. 새 monotonic game clock은0이다. 기존 auth serial suite·timeout/retry 정책을 유지한다. 같은DB/keys/currentUTC·실제 match/WS 후 kill→새context/page load→ID 전달 없이 결과 화면에서 unknown/null 본인 결과를 확인한다. known/없음/다른 계정·삭제도 검사하고 browser storage/URL에 ID가 없는지 확인한다. fixture SQL unknown이나 기존메모리ID lookup을 fresh browser 증거로 세지 않는다.

## 출구와 한계

실제 Red→Green·PG 최신 순서/정확90일/동일시각/삭제/오염·HTTP 권한/공유상한·UI lifecycle/8언어/실제 OS restart browser·coverage/docs/Mermaid/코드리뷰/CI를 확인한다. 그 뒤 상위18의 전체 인수 조건/통합 근거를 감사한다. #83 원인·OAuth42/하드웨어26/사람27·물리삭제/백업25는 별도다.
