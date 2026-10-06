# ADR0011: 검증된 규칙 snapshot과 순수 매치 전이

- 날짜: 2026-10-07
- 상태: 채택 — #7, FR03/04/05/15, TS04/05/06/08/09/14/27.

## 결정

`config/game-rules.toml`이 제품 숫자의 원천이다. Rust에서 unknown field·타입·범위·공개 전략/버전을 검증하고 canonical JSON의 BLAKE3 hash와 RNG/solver version을 RulesSnapshot에 묶는다. 주석·공백 변경은 hash를 바꾸지 않고 수치 변경은 바꾼다. 매치가 시작하면 snapshot은 고정하며 hot reload는 다음 매치에만 적용한다. public Rust 타입에서 TS를 생성하고 TS에 규칙 숫자를 복제하지 않는다.

RuleEngine은 Board·snapshot·서버 수신 tick과 명령을 받는 순수 도메인이다. 시간/난수/DB/WS를 직접 읽지 않는다. 같은 Board를 두 개인 관측에 연결하고 공격 전 zero opening을 공개하며 초기 gauge0으로 시작한다. 서버 actor는 수신 tick/seat/sequence 순서로 직렬화하며 batch API도 이 순서로 정렬한다.

zero opening만으로 미인증 판을 수락하지 않는다. 시작 생성 worker에서 일반 노게스 인증도 확인하고 막힌 판·budget을 거절한다. 초기 자동 flood가 두 사람의 모든 안전칸을 이미 열었다면 시작 tick에 clear 무승부다. 종료 후 공개 clock/stun 값은 종료 tick에 고정한다.

명령 캐시는 seat와 command ID에 묶고 동일 payload/seq/epoch 재전송에는 원래 ack를 반환한다. ID 충돌은 거절한다. 처음 처리한 유효 envelope는 행동이 거절돼도 sequence를 소비하며 projection은 따로 현재 상태로 만든다. config의 최대8192개 명령/seat 뒤에는 새 명령을 거절하여 무한 메모리를 방지한다. #16 rate limiter와 frame limit을 대체하지 않는다. 기절은 max(기존 종료 tick, 새 종료 tick), deadline 동일 tick은 먼저 종료, clear는 전이 후 즉시 확정한다.

열린/flagged 셀 재열기·중복 command는 safe/gauge를 더하지 않는다. 후보 없음·capacity·오래된 proof·예산 거절은 공격 unavailable 하나로 응답하고 gauge를 보존한다. 공격의 target은 서버가 닫힌 인증 후보 중 셀/delta 정렬 순서로 선택한다. 정확한 지목은 원래 값을 복원·overlay 제거·공격자 gauge0/2초 stun, 오지목은 본인3초 stun이다. opened 숫자만 지목할 수 있다. 지뢰는 공개하되 진행/gauge를 주지 않고3초 stun이다.

추론은 별도 AnalysisJob으로 실행한다. 도메인 전이는 공개 변경 observe만 수행하고 기존 immutable proof를 보존한다. 결과는 해당 seat revision이 유지될 때만 적용한다. 순수 도메인 job을 bounded CPU worker에 연결하는 작업은 #16이며 입력 처리에서 긴 DFS를 실행하지 않는다.

`advance(now)`는 외부 Clock을 사용하며 terminal 후보의 가장 이른 tick으로 판정한다. 둘 모두 끊겼다면 두 유예 만료까지 기다려 abandon하고, 한 명이 연결된 상태에서 다른 유예가 만료되면 forfeit다. 두 사람이 모두 끊긴 상태에서 한 사람만 먼저 유예가 만료됐다는 이유로 연결된 승자를 만들지 않는다. reconnect 시각이 유예 deadline과 같으면 늦었다. 서버 장애는 abort이며 완료/순위 카운트에 들어가지 않는다. 인증/WS epoch 회전은 #18 경계와 연결한다.

공개 projection은 본인 관측·flag·gauge·stun·숫자 이력과 상대 safe progress/stun만 포함한다. Board·seed·overlay·source·논리 truth·후보·상대 셀은 projection에 없다. 정확한 지목 ack는 해당 행동의 결과이며 지목 전 lie 표시를 추가하지 않는다.

공개 revision도 플레이어마다 분리한다. 숨은 공격 등록과 상대 flag로 defender의 revision이 늘어나지 않는다. 같은 tick에서 공격 전후 전체 defender projection이 일치하는 테스트로 숫자 필드 외의 시점 누출도 검사한다. 분석용 숫자 revision과 transport ingress 순서를 이 공개 카운터로 내보내지 않는다.

## 확인 근거

구성 parser는 [TOML Rust 공식 문서](https://docs.rs/toml/latest/toml/)와 [Serde unknown-field 계약](https://serde.rs/attributes.html), hash는 [BLAKE3 공식 Rust API](https://docs.rs/blake3/latest/blake3/)를 따른다. 최신 라이브러리를 버전 인자 없이 추가하고 lockfile에 기록한다. 명령8192 한계는 네트워크20/s·4분+30초유예의 최대5400보다 크며 메모리/동접 검증에서 #18/#26이 평가한다.
