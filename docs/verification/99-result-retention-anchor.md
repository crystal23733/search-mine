# #99 결과 보존 기준 검증

M3 · FR14/16, NFR01/02/05/07/08 · TS15/16/20/21/31/35/36. [ADR0045](../adr/0045-result-retention-anchor.md), 비교 기준 developecb6f21·선행97/PR98 병합·종료.

## 실제 Red → Green

실제 PG17.4에서 새 테스트3개가 모두42703(새 컬럼 없음)으로 실패했다. migration만 추가한 실행은1통과2실패: 정상 writer가 anchor=None으로 저장했고 기존 reader는 최근 저장된 결과의90일 지난 anchor를 노출했다. writer/reader 최소 수정 후 전체 실제 DB41개5.62초·ignored0 통과. schema 실패와 의미 있는 행동 실패를 구분한다.

nullable anchor를 recorded_at으로 backfill하고 유한 epoch0~9999년/recorded_at 이하 CHECK와 표현식 index를 추가했다. 이전 migration checksum·기존 INSERT·공개 DTO·seed 계산은 유지한다. legacy NULL은 실제 recorded_at으로 fallback한다. 기존 실제 reader의 known JSON을 capture한 golden과 historical SQL INSERT→forward migration으로 구 계약을 확인한다. 최신 writer가 과거 schema에서 작동한다고 가정하지 않는다. 지원 범위 밖 기존 오염 행은 migration 검증 실패로 중단하며 시각을 조작해 복구하지 않는다.

최근 저장/오래된 anchor의7775999/7776000/7776001초·NULL fallback·DST·미래 저장/역행/음수/무한/상한 CHECK·normal/owned writer의 시각 불변 duplicate·삭제 참여자 비복원·공개 JSON 시각 비노출을 실제 PG로 검사했다. 기존 seed7 calendar days 계산을 바꾸지 않았고 새 elapsed7일 보장을 달성했다고 주장하지 않는다.

## 실행 검증

- Rustfmt/Clippy workspace all-targets·workspace297통과(기본 실행의 DB41ignored는 별도 실제 DB 검사로 확인).
- 정식 migration CLI 성공 후 실제 PG/HTTPS 본인 known/unknown 결과 PC1440/mobile390와 철회 권한5개33.4초·workers1·재시도0 통과. 모든 기존 최소 DTO/타인404/503/8언어/storage0/keyboard 단언을 유지했다.
- 실제 DB 포함 서버 coverage 실행 성공. 아래 수치는 entry point를 포함한 측정값이다.
- docs149/21product18design36trace·Mermaid34실패0·git diff --check 통과.

| 범위 | 실제 line coverage |
|---|---|
| Auth domain | 385/387 · 99.48% |
| Match domain | 287/289 · 99.31% |
| Lobby domain | 362/368 · 98.37% |
| Result domain | 58/58 · 100.00% |
| 전체 서버 | 6324/6814 · 92.81% |

## 코드 리뷰와 제한

pm-ai-shipping:code-review correctness/changes, baseline ecb6f21. AuthClock→SQL anchor/recorded_at→COALESCE 조회, UUID retry→기존 시각/삭제 비복원, FK/계정별 projection→최소 공개 DTO를 함께 검토했다. 요청 저장 시각과 실제 원래 시각이 달라지는 duplicate 및 최근 저장/과거 anchor를 강제로 비교했다. 현재 변경에서 확인한 미해결 결함은 없다. 의존성/lockfile/프로토콜/TS/UI/게임 규칙은 변경하지 않았다.

전체 로컬 browser90개는 이번 DB 전용 변경에서 반복하지 않았다. 선행97의90개workers2/retries0 성공과 앞선 두 지연 관측은 해당 검증 문서에 보존했으며 이번 실행으로 계산하지 않는다. 최신 전체 CI 결과는 원격 완료 뒤 기록한다. #83 원인, 실제 하드웨어26/사람27/OAuth키42는 별도다. journal/admission/startup abort·실제 OS kill-restart·새로고침 발견 및 물리 삭제25는 후속이며 상위18은 OPEN이다.

## 원격 완료

[PR100](https://github.com/crystal23733/search-mine/pull/100)은2026-10-10T13:53:20Z developbd1f48fe33b91f6daa6dcc16b241e08d5856efe3에 병합됐고99는13:53:21Z 종료됐다. 최신head15fa079/Ready/CLEAN·최종본문 뒤8rollup SUCCESS 확인 후 exact-head squash. Product38056987563: 실제 PG18.6 DB41ignored0(9.98초/coverage9.14초),199unit35파일·웹87.94%,90browser3.7분재시도0·서버92.81%. 로컬 전체browser를 반복한 것으로 보고하지 않는다.
