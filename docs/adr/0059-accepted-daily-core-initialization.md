# ADR0059 — 서버 확정 규칙을 사용하는 데일리 WASM 초기화

2026-10-12 · M3/[#126](https://github.com/crystal23733/search-mine/issues/126) · 상위19.
FR09/10·NFR01/02/05/07/08·TS17/18/21/31/35/36. 사용자 승인·순차 검토/병합 위임 범위의 결정이다. 선행124/PR125 병합·CLOSED를 확인했다.

## 배경

서버 start/definition은 최초 게시한 accepted RulesSnapshot을 반환한다. 배포된 bundled 규칙과 accepted countdown/hash가 달라도 현재 DailySession/WorkerDailyCore는 bundled만 사용한다. 공식 도전을 연결하기 전에 accepted snapshot으로 동일한 metadata/seed/판을 시작할 경계가 필요하다.

## 결정

Native DailySession::with_rules(date,version,snapshot)와 from_replay_with_rules(source,snapshot)을 제공한다. new/from_replay는 bundled에 위임하여 기존 무계정 연습과 오프라인 복원을 유지한다. Rust DailyPuzzle::with_rules가 기존 RulesSnapshot::verify와 DailyMetadata::derive를 실행한다. JSON 초기화는 UTF-8 최대16384바이트의 closed RulesSnapshot을 받아 malformed/hash/알 수 없는 필드·solver/rng 버전·잘못된 규칙을 malformed로, 미지원 seed_version을 unsupported_version으로 거절한다. 생성 실패는 unavailable이며 기본 규칙으로 fallback하지 않는다.

WASM DailySession.withRules(date,seed_version,rules_json) named factory는 같은 Native 경계를 사용한다. f64 seed_version은 기존 strict 검사를 유지한다. DailyCore.init의 선택적 RulesSnapshot과 worker daily-init가 snapshot을 전달하며 생략한 두 인자 init는 bundled다. 생성된 Rust→TS RulesSnapshot을 재사용하고 WASM bindings는 wasm:generate로 갱신한다. 규칙·hash·seed 알고리즘을 JS에서 구현하지 않는다. 실패한 init는 세션을 만들지 않으며 snapshot은 not_started, 이후 유효한 init는 가능하다.

이 경계는 공개 데일리 전용이다. online seed/정답을 노출하지 않는다. 클라이언트가 snapshot을 넘길 수 있다는 것은 서버의 publication·ticket·시간·공식 제출 권위를 바꾸지 않는다. 후속 공식 controller는 서버 accepted metadata/rules와 반환한 view를 비교하고 불일치 시 시작을 중단해야 한다. replay 복원도 전달 snapshot의 metadata와 완전히 일치해야 한다.

replay는 기존 closed protocol의 최대2MiB/8192개 wire 상한과 accepted max_commands_per_seat를 모두 유지한다. accepted 규칙이 wire 상한보다 큰 명령 수를 허용해도 현 프로토콜이 그보다 긴 replay를 수신한다고 주장하지 않는다. 더 작은 accepted 명령 상한은 Native adapter가 적용한다.

## 검증과 후속

기존 production worker가 추가 snapshot을 무시하여 bundled metadata/countdown을 반환하는 실제 행동 Red를 먼저 실행한다. 테스트용 golden은 Rust core의 countdown5000/hash·seed가 다른 snapshot으로 생성한다. Native·실WASM·production worker의 전체 초기 projection·완료 step/replay와 accepted replay 복원, 손상 hash/unknown field/버전/초과16KiB/no fallback·거절 뒤 재초기화를 검증한다. 기존 bundled parity·무계정/오프라인·전체 회귀, 실제 커버리지 게이트·문서/Mermaid·코드 리뷰·최신CI·정확head 병합/종료를 확인한다.

새 라이브러리·DB 변경·OAuth 키는 필요하지 않다. 명시 공식 시작/heartbeat/순위 UI·session-bound pending 전송·실제 HTTPS UI와 상위19 통합은 후속이다. 외부42·운영25/26·사람27·미확정83은 별도다.
