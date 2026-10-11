# ADR0050 · bounded native 데일리 리플레이 검증

- 상태: 채택 · 2026-10-11 · M3 #109 / 상위 #19
- 선행: #107/PR108 develop6a2be6d 병합·107 CLOSED 및 #18 CLOSED 확인
- 추적: FR09/10 · NFR01/02/05/07/08 · TS17/18/20/21/31/35/36

## 결정과 신뢰 경계

기존 [ADR0017](0017-deterministic-solo-daily-and-local-records.md)의 공개 솔로 데일리를 서버에서 같은 Rust `DailyPuzzle`로 재생한다. 서버가 선택한 날짜/seed version/검증된 `RulesSnapshot`을 불변 `PublishedDaily`로 고정한다. metadata의 날짜·mode·seed·rules hash·solver/RNG/seed version 전체를 비교하며, 클라이언트가 보낸 rules로 definition을 만들지 않는다. 보존 snapshot을 현재 bundled rules로 바꾸지 않는다. 현재 알고리즘과 replay wire 명령 상한 안의 snapshot만 구성할 수 있다. 임의 과거 알고리즘 호환이나7일 verifier 저장이 이 foundation에서 완료된 것은 아니다.

`daily/model.rs`는 published definition·재생 evidence·오류·동기 `ReplayVerifier` port를 소유한다. `daily/replay.rs`의 native 어댑터가 기존 protocol의 strict `decode_replay`와 공유 코어를 호출한다. `daily/bounded.rs`는 Tokio 실행·capacity·기한만 담당한다. 도메인은 HTTP/DB/Tokio를 모른다. WASM crate에 의존하거나 생성/규칙/통계를 별도 구현하지 않는다. DTO→core action 변환은 Open/Flag를 전달하는 어댑터 책임이다.

완료 evidence는 실제 Clear/SeatOne일 때만 생성하며 metadata·opened_safe·mistakes·재생 elapsed만 가진다. timeout/미완료는 `Incomplete`다. account/session·board·숨은 정답·공식 rank/시간 trust를 담지 않는다. 클라이언트의 주장 통계/elapsed를 evidence로 복사하지 않는다. 재생 elapsed도 client 입력 시각에서 계산된 개인 참고값이며 공식 시간 증거가 아니다. 수정한 로그가 일관된 완료를 만들 수도 있으므로 replay 검증으로 인간 풀이 또는 실제 오프라인 경과시간을 증명한다고 표현하지 않는다. official 첫 완료/시간은 후속 repository와 server ticket/heartbeat가 결정한다.

## 입력과 중복

기존2MiB UTF-8 byte 상한·strict version/shape/숫자와 wire 명령 수 상한을 재사용한다. definition의 명령 상한도 적용한다. 모든 metadata 필드, 입력 시각의 비감소/마지막 시각 이하, 허용 Open/Flag와 정적 cell 범위를 검사한다. wire command_id/client_seq는 양수이며 core의 실제 command cache·seq·conflict 계약을 따른다. 새 seq는 단조 증가면 되고 연속일 필요는 없다. 동일 ID/seq/action 중복은 원본 cached ACK이며 효과/통계를 다시 만들지 않는다. ID 충돌/순번/시각/명령 상한 위반은 유효 완료로 통과시키지 않는다.

WASM은 거부된 클릭도 replay에 기록한다. NotStarted/Stunned/AlreadyOpen/Flagged/Finished와 열린 유효 칸에 대한 Flag의 InvalidCell은 정상 입력일 수 있으므로 모든 rejected ACK를 위조로 처리하지 않는다. 정적 범위·mode·identity 검증과 core 거부를 구분한다. 종료 뒤 final_time을 진행하는 기존 core 동작도 보존하며 임의의 추가 절대 시각 상한을 만들지 않는다. 만료 판정과 시간 순위는 별도 후속 권위다.

## CPU 실행과 수명

동시에 최대2개의 실제 blocking 검증만 허용하고 `try_acquire_owned`로 초과 요청을 즉시 Busy 처리한다. byte 상한을 admission/문자열 복사 전에 확인하고, JSON parse·생성·재생은 `spawn_blocking` 안에서 수행한다. caller 응답 기한은2초다. `OwnedSemaphorePermit`을 caller future가 아닌 blocking closure가 소유하여 timeout/cancel로 future가 사라져도 실제 작업 종료 전 capacity를 재사용하지 않는다. queued task도 같은 한도에 포함한다. panic은 Unavailable이고 unwind/정상 종료 때 capacity를 반환한다. 늦은 결과는 폐기되며 검증기에는 DB/write 부작용이 없다. 공유 생성기의 기존 탐색 예산도 유지한다.

## 검증과 종료

실제 native/WASM golden replay를 서버 어댑터로 재생한다. metadata 각 필드·shape/version/size/count/time/action/cell/identity 변조, incomplete/timeout, exact duplicate/정상 rejected 클릭, retained snapshot/current fallback 거절을 검사한다. fake blocking verifier로 actual permit·timeout/cancel/late completion/panic/과부하·큰 입력의 admission 순서를 제어한다. 테스트 gate는 실패 때도 release하여 runtime을 멈추지 않는다.

시나리오→실제 Red→Green→리팩터·관련 Rust/생성물/전체 회귀/실DB gates·docs/Mermaid·fresh coverage를 실행한다. `scripts/check_auth_coverage.py`에 Daily model95% gate를 추가하고 기존 도메인/entry point/전체80%를 유지한다. 테스트 본문은 기존처럼 tests/unit 경로에 두며 제품 coverage 제외를 늘리지 않는다. [검증109](../verification/109-bounded-native-daily-replay.md)에 실제 결과/미실행을 구분한다. 최신 head/CI와 코드 리뷰 뒤 develop 병합·109 CLOSED를 확인한다.

다음 #19 하위는 실제PG published snapshot/첫 유효 완료/attempt idempotency·삭제/보존 → 인증 HTTP/서버 timing/leaderboard → 명시 로그인 제출/8언어 UI·실제 HTTPS 출구다. 사용자 위임 범위의 구현 분할이며 새 외부 계정이나 키를 요구하지 않는다. 상위19와 M3 전체는 이 foundation으로 닫지 않는다.

Tokio의 [spawn_blocking 계약](https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html)은 시작한 blocking 작업을 abort로 중단할 수 없으며 CPU 동시 실행 제한을 별도로 두도록 설명한다. [owned semaphore](https://docs.rs/tokio/latest/tokio/sync/struct.Semaphore.html#method.try_acquire_owned)와 [timeout](https://docs.rs/tokio/latest/tokio/time/fn.timeout.html)의 수명에 맞춰 실제 permit을 closure 안에 둔다. 동시 두 definition의 완료 순서를 반대로 제어하고 각 caller가 자신의 결과만 받는 시나리오도 검증한다. 임의 OS 스케줄링/미니PC 성능 달성 주장은 아니다.

첫 CI의 기존 owner-loss 프로세스 종료 검사 실패를 따라 [ADR0051](0051-fatal-runtime-shutdown.md)의 fatal runtime 수명 경계를 별도로 검증한다. 이 보정은 caller timeout에 CPU permit을 반환하는 것이 아니다. 실제 프로세스의 치명 오류 반환만 blocking 작업 완료를 기다리지 않도록 하며 정상 graceful 경로는 유지한다. 원격 실패 원인은 추가 관측 전 확정하지 않는다.
