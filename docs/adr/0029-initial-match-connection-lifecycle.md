# ADR0029: 배정 후 최초 게임 연결과 세션 수명

2026-10-09 · #62/M3 · 상위 #17 · FR02/06/07/14/16, NFR02 · TS10/12/15/16/20/21/35. 선행 #60/PR61 develop904de85 병합·종료.

## 결정과 규칙 근거

[시작 전 끊김은 취소](../design/04-game-rules-spec.md) 규칙을 아직 한 번도 연결하지 않은 human seat에도 적용한다. 준비 완료/registry 배정에서 시작하는 기존 전체 countdown과 Rust rules snapshot의 수치는 유지한다. 필요한 human이 시작 기한까지 모두 실제 WS를 연결하지 않으면 그 논리 기한에 Cancelled(completed=false)로 종료한다. 늦게 처리한 timer/input/attach라도 미연결 상태에서 Playing을 시작하지 않는다. 없는 상대에게240초 정상 경기를 주거나 자동 봇으로 바꾸지 않는다.

RuleEngine에 단일 시작 전 취소 연산과 시작 기한 accessor를 둔다. 취소는 현재 core 시각이 시작 전이고 취소 시각이 현재~시작 기한 안일 때만 허용한다. 이미 진행/종료된 판을 취소할 수 없다. actor는 초기 admission이 남아 있는 동안 모든 ingress 처리 전에 확인하여 core가 시작 기한을 지나도록 advance하기 전에 취소한다. 늦은 수신에서는 receipt time을 조작하지 않고 core의 논리 시작 기한으로 취소 결과를 확정한다. client action이나 TS에 새 규칙/취소 권한을 만들지 않는다.

## 입장과 권한 소유

MatchAdmission은 별도 로비 authority registry/clock과 seat별 private lease를 담고 Serialize/Debug가 없다. create_admitted는 countdown 상태·각 human의 account/seat·bot 형태·별도 namespace를 확인한다. 같은 두 참여자의 권한 lock 아래 registry 생성과 actor 소유권 이전을 직렬화한다. 내부 state를 직접 만드는 기존 create/create_with_bot port는 fixture/내부 구성용으로 보존하고 공개 로비는 create_admitted만 사용한다.

actor는 각 인간의 첫 게임 연결까지 로비 lease를 소유하고 매 ingress에서 철회/정확 expiry를 검사한다. 최초 attach는 해당 로비 권한→게임 권한→core attach 순서로 검증한다. 성공한 reply와 살아 있는 송신 sink를 확인한 뒤 그 seat의 초기 lease를 반환한다. 마지막 필요 인간이 연결하면 초기 gate를 제거하며 이후 게임 authority/epoch와 기존30초 재접속 계약이 권위를 가진다. 초기 lease가 남은 동안 revoke/expiry가 생기면 시작 전 취소한다. bot은 account/lease를 갖지 않고 기존 core bot driver를 사용한다.

잠금은 Lobby→로비 authority→registry 또는 로비 authority→게임 authority→pure state 방향이다. 합성 invalidator는 각 namespace lock을 따로 반환하고 다른 namespace를 기다리는 동안 앞선 lock을 보유하지 않는다. lease의 마지막 Drop은 해당 authority guard 밖에서 수행한다. 결과 저장/송신/정리·실제 worker permit의 수명은 기존 actor를 재사용하며 초기 취소는 승패/광고 완료로 계산하지 않는다.

## 검증과 남은 범위

core의 정확 시작 기한·역행/시작 후/종료 취소 거절을 실제 Red→Green으로 확인한다. admitted actor는 첫 연결 전 철회/만료와 Drop·human2명/봇1명·기한1ms 전/정확 기한·지연 tick/late attach·정상 공개 입력·두 namespace guard/seat mismatch·단일 취소 결과와 account 반환을 검사한다. 기존 실제 WS·PostgreSQL·native/WASM·coverage도 확인한다.

공개 HTTP Origin/CSRF/열거 admission/main과 OAuth 초대/8언어 화면은 다음 #17 하위 작업이다. 상위17은 아직 OPEN이다. 실제 키42·미니PC26·사람27은 별도 실제 출구다. 초기 gate를 새 공개 DTO로 표현하거나 정답/세션을 클라이언트에 보내지 않는다.
