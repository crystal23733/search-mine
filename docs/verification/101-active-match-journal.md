# #101 영속 active journal 검증

M3 · FR14/16, NFR01/02/05/07/08 · TS15/16/20/21/31/35/36 · [ADR0046](../adr/0046-active-match-journal.md). 비교 기준 developbd1f48fe, 선행99/PR100 병합·종료 뒤 시작.

## 결과와 변경 코드

최소 active parent(UUID/hash/owner/admitted UTC)와 child(seat/account FK cascade/is_bot)를 추가했다. secret/seed/board/입력/최종 통계/session/provider/nickname을 복사하지 않는다. ActiveMatch/AdmissionJournal port와 별도 PgJournalRuntime을 같은 raw 소유 연결/기존 lock key/16queue/2초/heartbeat worker에 연결했다. 내부 mode는 typed 구성이고 공개 bool bypass가 아니다.

register/discard는 같은 owner/UUID/hash/원래 참여자의 원자·중복·삭제 계약을 따른다. strict final은 살아남은 참여자만 저장하고 결과+active 삭제를 같은 transaction에 둔다. admitted 보존 기준/실제 recorded_at·seed604800초 저장 경계를 유지한다. 이미 저장된 seed/백업의 물리 삭제는25의 출구다. typed claim은 잠금 획득2초 뒤 사전 조회 포함 복구 전체30초·batch64/각batch2초/최대10000으로 unknown server_failure/abort/false/null details/null seed를 저장해야 healthy를 반환한다. 시각/통계를 추정하지 않는다.

## 실제 Red → Green

typed stub/schema 전8실패(42P01/입장 unavailable/unjournalled Saved/미복구 healthy). migration 뒤 schema는 통과했고 저장·복구·취소가 실패했다. 미구현 실패 신호를 기다리던 최초 테스트의 무기한 관찰을3초로 제한했다(제품 기한 변경 없음). 완료한 실제 행동 Red는1통과7실패3.77초. 구현 후7통과1실패는 삭제된 참여자가 있는 이미 final UUID의 오류 우선순위였고 final 존재를 먼저 거절하도록 정리했다.

전체49개7.57초 통과 후 중요한 경합을 추가했다. 최종 실제 PG17.4 전체54개7.91초·ignored0 통과. 새 journal 검사는13개이며 최소 schema/정수 UTC/제약·등록 부분 실패/원자성·불변 중복/삭제·strict final rollback/살아남은 참여자·66개 batch 복구·10001초과 거절·역행/DB실패/active+final 오염·seed604799/604800/604801·보존90일·owner 배타성을 포함한다.

queued 취소는 해당 SQL을 skip하고 owner를 유지한다. SQL 시작 후 취소는 owner를 종료한다. commit된 Saved 및 Duplicate ACK를 아직 소비하지 않은 future를 실제 같은 worker의 후속 명령으로 확인한 뒤 drop해 owner failure→startup abort를 검증했다. Duplicate를 추정 discard하지 않는다. 계정 삭제가 lock을 잡은 동안 복구를 시작하고 삭제 commit 후 살아남은 participant만 저장하는 실제 경합도 통과했다.

## 실행 검증과 코드 리뷰

- 정식 migration CLI 성공. scripts/check.ps1 Exit0: fmt/Clippy/workspace297·WASM/생성타입/fixture/token/format/lint/typecheck/웹199unit35파일(line87.94%)/build·실제 DB/HTTPS 전체90browser2.8분재시도0·docs151/Mermaid34실패0 통과.
- 실제 DB54개를 포함한 서버 coverage 성공. 도메인95/전체80 기준을 아래와 같이 충족했다. journal wrapper16/16·100%.
- git diff --check 및 staged 비밀/원문/의존성/ignored review 제외를 확인한다. 최신 CI와 PR 병합 상태는 완료 후 기록한다.

| 범위 | 실제 line coverage |
|---|---|
| Auth domain | 385/387 · 99.48% |
| Match domain | 287/289 · 99.31% |
| Lobby domain | 362/368 · 98.37% |
| Result domain | 58/58 · 100.00% |
| 전체 서버 | 6656/7152 · 93.06% |
| Journal SQL | 208/212 · 98.11% |
| Owner worker | 227/232 · 97.84% |

pm-ai-shipping:code-review correctness/changes, baseline bd1f48fe. UUID/hash/participants의 journal 원본→strict final, key-share→participant 재조회/잠금→삭제 비복원, 같은 raw lock/write/recovery, CAS queued/started/observed→watch 종료→대체 owner를 함께 검토했다. 기존 normal worker/main owner loss·기존 WS 종료 회귀도 실제 DB54개에 포함한다. 현재 범위에서 확인한 미해결 결함 없음. 규칙/프로토콜/TS/UI/OAuth 필드/의존성/lockfile은 바꾸지 않았다.

## 남은 통합

현재 공개 lobby/main/HTTPS는 기존 normal writer를 사용한다. 저장 foundation의 독립 검증이며 사용자 OS 재시작 복구 완료가 아니다. 비동기 admission의 durable ACK/원래 lease·reservation 재검증/receipt 확정·명시 discard/예기치 않은 drop 보상과 main/fixture strict 활성화, 실제 OS kill/restart·새로고침 결과 발견은 다음 통합이다. 상위18/원인 미확정83/외부42·26·27/물리삭제25를 유지한다. 이번 전체browser 성공이83의 원인 해결을 뜻하지 않는다.


## 원격 완료

PR102/head19ff8b5는 최신8checks SUCCESS/Ready/CLEAN 후 2026-10-10T17:04:15Z squash 병합했다. develop e5b306231e886a881101684ca357efb956aee7f6, 이슈101 CLOSED 17:04:16Z 확인. Product38059297291 실제 PG18.6 DB54/ignored0·199unit35파일/웹87.94%·90browser4.0분재시도0·서버93.06%/각domain95%이상. 로컬 전체 scripts/check Exit0·실DB54 및 코드 리뷰 완료. 공개 통합은 후속103이다.
