# #97 온라인 저장 소유 연결 검증

M3 · FR14/16, NFR01/02/05/07/08 · TS15/16/20/21/31/35/36. [ADR0044](../adr/0044-online-storage-owner.md), 기준 developfd29c11·선행95/PR96 병합·종료.

## Red와 구현

실제 PostgreSQL17.4에서 같은 결과 relation을 쓰는 main 두 프로세스를 실행했다. 기존 구현은 두 번째 서버가 종료되지 않아1개5.78초 실패했고, 단일 소유 연결 구현 뒤 같은 시나리오가1개2.90초 통과했다. 최초 module 경로 누락의 compile failure는 행동 Red와 구분한다.

PgResultRuntime/Health는 같은 raw 연결의 advisory lock·결과 저장·heartbeat와 대기16개/큐 포함2초 deadline을 관리한다. main과 실제 HTTPS fixture에 구성했고 ready의 DB 검사 전후 상태 확인과 서버 수명 감시를 연결했다. 기존 ResultRepository/FinishedMatch/known JSON/불변 duplicate/삭제 비복원 동작을 유지한다. 도메인 규칙·기한·의존성·lockfile·migration 변경은 없다.

## 실행 결과

- 실제 PG 소유권6개4.44초 통과: 두 main 거절·정상 drop/재획득·backend 종료 뒤 이전 handle 저장 불가·SQL 기한 초과/rollback·큐16 포화/시작 전 취소·시작 후 caller 취소 commit/duplicate·Malformed·삭제 비복원.
- 실제 main의 두 human 매치와 인증 WS snapshot 뒤 owner backend를 종료했다. main 비정상 종료·HTTP 중단·기존 WS 종료를 확인했다. DB backend 종료를 OS kill/restart 결과 복구라고 보고하지 않는다.
- worker 상태 채널 종료/정상 서버 완료2unit·Clippy 통과. 기존 무계정 main·잘못된 config·DB 불가 startup3개1.04초 통과. 인증 bootstrap/CSRF gate는 실제 DB main 검증으로 옮겼다.
- 전체 서버 coverage 실행에 실제 PG38개ignored0/14.98초와 결과 HTTP14개0.85초를 포함했다.

| 범위 | 실제 line coverage |
|---|---|
| Auth domain | 385/387 · 99.48% |
| Match domain | 287/289 · 99.31% |
| Lobby domain | 362/368 · 98.37% |
| Result domain | 58/58 · 100% |
| 전체 서버 | 6324/6814 · 92.81% |

초기 큐 테스트는 취소 직후 동기 drain을 가정해 Unavailable을 받았다. 기존1초 내 retry로 실제 Duplicate를 확인하도록 순서를 교정했으며 제품 기한을 바꾸지 않았다. WS 추가 검사는 null-session CSRF 재사용으로403을 받아 실제 세션별 fresh bootstrap proof로 교정했다. 제품 CSRF를 완화하지 않았다.

## 전체 검사와 미확정 관측

scripts/check.ps1은 Rustfmt/Clippy/workspace/WASM/types/fixtures/tokens/format/lint/typecheck/199unit35파일14.13초·웹87.94%/build를 통과한 뒤 최초 browser70통과1실패19미실행2.6분으로 종료했다. unknown1440의 본인 결과/8언어/screenshot 뒤 타인 새 페이지의 Google 버튼 대기가30초 timeout이었다. 최초 trace/error는 .tmp/result97-first-browser-*에 보존했다. finally context-close 오류와 최초 대기를 구분한다. 새 페이지의 정적 assets200 뒤 bootstrap 요청/console 오류는 관측하지 못했고 원인은 미확정이다. 기존 CDP/83 SW 원인을 해결했다고 주장하지 않는다.

같은 unknown PC/mobile repeat2/workers1/retries0은4개25.1초 통과했다. 최종 전체 browser/문서 재검사와 최신 CI는 완료 후 기록한다. journal/admission/startup abort·실제 OS kill/restart 결과 복구·새로고침 본인 결과 발견 및 외부42/26/27은 후속이다.

두 번째 전체 browser는87통과1실패2미실행6.4분이었다. 계정/결과 검사는 모두 통과했고 기존 offline-chromium의 초기 캐시 준비 문구5초 대기가 실패했다. 자료는 .tmp/result97-full-recheck에 보존했다. 2-worker 조건에서 전체 비교 검증하며 제품/Playwright config/timeout/retry는 변경하지 않았다. 이 조건은 CI와 같으며 기존 테스트 안의 실제 동시 탭/WS 행동을 유지한다. 소유 연결 자체 line coverage는133/136·97.79%다.

최종 실제 PG17.4/HTTPS 전체 browser는 `pnpm exec playwright test --workers 2 --retries 0 --output .tmp/result97-full-workers2`로90개8.0분·재시도0 통과했다. CI와 같은2-worker 조건이며 제품/Playwright config/timeout/retry는 변경하지 않았다. 이전 두 전체 실패와 별도로 기록하며 원인을 해결했다고 주장하지 않는다. docs147/Mermaid34실패0·최종 코드 리뷰·staged diff/비밀·원문/ignored review 제외 확인. 원격 CI/PR 병합은 완료 후 기록한다.

## 원격 완료

[PR98](https://github.com/crystal23733/search-mine/pull/98)은2026-10-10T13:28:21Z developecb6f21c98ee368bc3ef58723c4b1ae2ef261358에 병합됐고97은13:28:22Z 종료됐다. 최신head667e9a3/Ready/CLEAN·본문 후 Repository 검사까지8개 rollup SUCCESS 확인 후 exact-head squash. Product38055227162: PG18.6 DB38ignored0(8.53초/coverage9.58초),199unit35파일·웹87.94%,90browser5.4분재시도0·Auth99.48/Match99.31/Lobby98.37/Result100%·서버6324/681492.81%. 로컬 두 지연과90개8.0분workers2retry0 결과를 구분한다. 원인 미확정 자료를 보존하며 journal/OS kill-restart 복구는 후속이다.
