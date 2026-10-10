# #103 · durable lobby 입장과 실제 재시작 복구 검증

실제 lobby/main/HTTPS가 journal ACK 이후에만 대전을 공개한다. 서버 OS 강제 종료 뒤 새 main은 serving 전에 진행 중이던 판을 미확인 중단으로 저장한다. [ADR0047](../adr/0047-durable-lobby-admission.md) · FR14/16 · NFR01/02/05/07/08 · TS15/16/20/21/31/35/36. 선행101/PR102 develop e5b3062 병합·종료 확인 뒤 구현했다.

## 구현과 행동

- core PreparedEngine 실제 hash→새 UUID/원래 예약·players·leases→bounded async register→durable ACK→동일 key/token/fresh authority→전체 countdown/메모리 commit. lobby/authority mutex 아래 DB await 없음.
- 명시 LobbyMatchServices/필수 AdmissionJournal; main/HTTPS와 실제 DB lobby가 같은 Arc<PgJournalRuntime>을 결과와 입장에 사용한다. 공개 legacy/noop/선택적 우회 없음.
- blocking 준비부터 receipt commit/discard까지 workers permit 유지. 정상 cancel/logout/expiry/서비스 drop은 register 결과를 관측한 뒤 해당 UUID를 명시 discard한다. receiver.close/이미 받은 값/late send 반환의 양순서를 처리한다.
- 미확정 receipt의 예기치 않은 Drop/정리 실패는 기존 AbortHandle로 owner를 종료한다. actor 생성 직후 panic에서 무조건 journal을 지우지 않는다. worker만 watch sender를 소유한다.

## 실제 Red→Green과 회귀

core getter 누락 E0599 Red→getter5개 Green. journal을 호출하지 않는 기존 lobby에서 durable ACK 시나리오가 bounded2초 행동 Red. 구현 후20초 test delay가 실제5초 예약을 만료시키는 것을 확인하여3500ms로 보정했다. 정책 expiry/timeout/retry는 바꾸지 않았다. 최종 lobby30개/HTTP11개/receipt4개를 전체 검사에서 확인했다.

실제 PG17.4 전체58개/ignored0/7.41초, coverage 포함 재실행58개/10.20초. 새4개 공개 runtime 검사는 다음을 확인한다.

1. 실제 main 인증 HTTP pair/WS snapshot→Child.kill(Windows TerminateProcess/Unix SIGKILL)→기존 HTTP/WS 종료→같은 DB/keys 새 main→본인 unknown server_failure/abort/false/null elapsed/own/null seed·원래 admitted anchor. 타 계정404와 두 번째 kill/restart의 SQL/HTTP 불변도 확인했다.
2. 실제 core/actor가 저장한 최초 미접속 시작 취소 known 결과는 kill/restart 이후 전체 SQL/HTTP가 불변이다. 정상 gameplay clear·봇/사람 결과 저장은 기존 실제 HTTPS browser 회귀로 확인했다.
3. 복구 두 번째 participant SQL 실패는 transaction rollback/active 보존/프로세스 비정상 종료/HTTP serving0이다. 실패 제거 후 계정 삭제가 FK에 반영되며 재시작은 살아남은 참여자만 저장한다.
4. SQL 시작 후 normal cancel은 register future를 버리지 않는다. test-only transactional INSERT/DELETE audit가 같은 UUID의 등록/명시 삭제 두 commit을 증명한다. active0/final0·동일 owner PID와 ready 유지. 단순 count0을 완료 증거로 사용하지 않았다.

scripts/check.ps1 최종 Exit0: fmt/Clippy/workspace310·WASM/types/fixtures/tokens/format/lint/typecheck·199unit35파일/웹line87.94%·production build·전체90browser3.1분재시도0·docs153/Mermaid34실패0. 첫 script 실행은 PowerShell native stderr 리디렉션/Stop 충돌로 중단했으며 OS stdout/stderr 분리의 숨김 하위 PowerShell로 동일 script를 완료했다.

## 실제 coverage와 측정 산출물

CORE 2347/2399=97.83%, game438/440=99.55%. Auth385/387=99.48%, Match287/289=99.31%, Lobby362/368=98.37%, Result58/58=100%, 서버6769/7285=92.92%. domain95/전체80 목표 통과. admission orchestration93/98=94.90%, lobby service415/439=94.53%는 infrastructure 측정이며 domain과 구분한다. 미니 PC 성능은 #26이다.

첫 로컬 core 측정은 기존 coverage target에서2347/2823=83.14%로 Exit1. 실행된 getter를0으로 표시하고 game.rs를864줄로 중복 집계했다. 제품/테스트/95% 기준 변경 없이 새로운 CARGO_TARGET_DIR=.tmp/admission103-cov-clean으로 같은 명령을 실행하여97.83%/Exit0을 얻었다. 실패/성공 로그·JSON은 ignored .tmp에 보존하고 기존 부풀려진 측정을 통과로 사용하지 않는다. 서버 측정도 독립 산출물과 실제58DB를 사용했다.

## 코드 리뷰와 한계

pm-code-review correctness/changes, baseline e5b306231e886a881101684ca357efb956aee7f6를 사전 선언했다. 실제 hash≠bundled proposal, 두 pair 역순 완료/old UUID 취소, ACK gate/다른 요청 무차단, lease 교체/기한/철회, ready/late oneshot 양순서, receipt panic/no-delete/cleanup failure, same-owner strict main/recovery/erasure를 반례 실행으로 검토했다. 현재 범위의 근거 있는 미해결 correctness 결함 없음. latest CI/head/PR 병합은 원격 완료 후 기록한다.

새로고침으로 메모리 ID를 잃은 뒤 본인 결과 발견은 다음 인증된 bounded 조회/웹 작업이다. 일반 storage/URL에 online match ID를 저장하지 않는다. 상위18은 그 발견 및 전체 출구 감사 전 닫지 않는다. 기존83 원인 미확정·OAuth42/하드웨어26/사람27·seed/백업 물리삭제25는 별도다. 브라우저 성공을83 원인 해결로 보고하지 않는다. 규칙/프로토콜/개인정보 권한/의존성/locks 변경 없음.


## 원격 완료

PR104/headc5da7f3은본문갱신후최신8checks SUCCESS/Ready/CLEAN에서2026-10-10T17:38:09Z squash 병합했다. develop9eeb36c97ea368bd033a688f56974c96187af4a1·이슈103 CLOSED17:38:11Z를확인했다. Product38072018795/Repository38072018834 SUCCESS: 실제PG18.6 DB58/16.75초·coverage58/16.62초/ignored0·199unit35파일/웹87.94%·90browser3.4분재시도0·서버92.92%/각domain95이상/core95기준PASS. 다음105가새로고침뒤결과발견을연결한다.
