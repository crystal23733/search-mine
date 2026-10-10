# ADR0047 · durable 입장 확인과 실제 프로세스 복구

- 상태: 채택 · 2026-10-11 · #103, 선행101/PR102 병합·종료
- 추적: FR14/16, NFR01/02/05/07/08 · TS15/16/20/21/31/35/36

## 문제

ADR0046의 journal만으로는 공개 lobby가 입장 기록을 쓰지 않는다. matched/actor 공개 전에 durable ACK를 받고, 정상 취소를 저장 owner의 비정상 종료와 구분해야 한다. lobby/authority mutex 아래 DB를 기다리면 다른 요청과 철회가 막힌다.

## 결정

LobbyMatchServices는 registry/bots/필수 AdmissionJournal을 명시 DI한다. 공개 main과 실제 HTTPS fixture는 같은 Arc<PgJournalRuntime>을 결과 저장과 입장에 사용한다. 일반 저장 port는 독립 adapter 테스트에만 남기며 공개 실행의 noop/선택적 우회를 만들지 않는다.

준비 job이 실제 PreparedEngine의 적용 rules_hash getter, 새 UUID, 원래 reservation/key/players 및 authority leases를 소유한다. UUID/hash를 확정하여 register한 뒤 결과를 관측한다. DB await는 async job에서만 수행한다. durable ACK 전 actor/grid/matched는 공개하지 않는다. lobby mutex 아래 동일 reservation·players·원래 lease token과 fresh authority를 재검증하고 engine.start(now)→동일 UUID MatchState→create_admitted→policy.commit→receipt 확정을 await 없이 처리한다. countdown은 이 시각부터 전부 제공한다. 새로운 세션의 lease로 과거 준비를 승격하지 않는다.

workers permit은 blocking 준비부터 durable receipt의 commit/discard까지 유지하여 완료 대기/정리 수도 제한한다. blocking 준비가 아직 실행 중이면 취소해도 permit을 반환하지 않는다. 준비 전 취소는 기존대로 중단한다. register 시작 후 정상 cancel/logout/expiry/erasure는 register future를 버리지 않고 bounded 결과를 관측하여 해당 UUID를 명시 discard한다. discard ACK 후 receipt를 해제한다. 실패/포화로 정리를 확인하지 못하면 fail_closed로 owner worker를 종료한다.

완료 oneshot의 Receiver.close를 먼저 수행하고 이미 받은 receipt를 try_recv하여 정리한다. 닫힌 receiver로 Sender.send가 실패하면 반환된 receipt를 생산자가 정리한다. 정상 취소에서 try_recv→drop 사이의 send 경합으로 owner를 종료하지 않는다. 서비스 drop도 이 경로를 따른다. 런타임 종료 등 정리가 끝나지 못한 미확정 receipt Drop은 동기 AdmissionJournal::fail_closed로 owner를 종료한다. actor 생성 직후 panic의 receipt를 무조건 discard하면 살아 있는 actor의 기록을 지울 수 있으므로 예상하지 못한 Drop에서는 삭제하지 않는다. watch sender의 단일 소유는 유지한다.

입장 실패·stale reservation·fresh proof 교체·권한 철회는 actor를 만들지 않고 해당 receipt만 정리한다. actor 공개 이후의 정상 final은 journal과 결과를 원자 전환한다. owner 실패 시 기존 main serve guard가 프로세스를 종료한다. 새 main은 serving 전 startup recovery를 끝내며 unknown abort/null details/seed 없음/원래 retention anchor를 유지한다.

## 검증과 제한

gated journal로 ACK 전 비공개/다른 사용자 무차단·countdown·원래 hash/UUID/lease·늦은 취소/expiry/logout·서비스 drop·oneshot 경합·정리 실패 fail-closed를 행동으로 검증한다. core getter는 다른 유효 규칙과 실제 projection hash를 비교하고 native/WASM 계약·coverage를 검사한다.

실제 main 프로세스+HTTP 인증 세션+WS snapshot 뒤 Child.kill(Windows TerminateProcess/Unix SIGKILL)하고 같은 DB/keys로 새 프로세스를 띄워 본인 최소 결과·정확히 한 번·보존 기준을 확인한다. 정상 final 저장 후 kill/restart는 known 결과를 바꾸지 않아야 한다. DB backend 종료를 OS kill 증거로 계산하지 않는다. 실제 DB 원자 실패/삭제 경합은 journal 통합 검사와 공개 runtime 회귀로 확인한다.

새로고침 뒤 메모리 match ID 발견은 별도 인증된 bounded 본인 결과 발견 작업이다. 일반 storage/URL에 online ID를 저장하지 않는다. 상위18·원인 미확정83·외부42/26/27·물리삭제/백업25는 이 변경으로 종료하지 않는다.
