# 06. HTTP·WebSocket 프로토콜

> 대응: FR01/06/07/09/14, NFR02 · Rust public DTO → ts-rs → TS. 내부 Board와 public DTO는 분리한다.

## HTTP

| 경로 | 계약 |
|---|---|
| GET /api/v1/auth/providers | 서버에서 설정/검수된 제공자 목록만, client secret/endpoint 없음 |
| POST /api/v1/auth/{provider}/start | CSRF/Origin, login intent, one-use state·nonce·browser binding; authorize URL |
| GET /api/v1/auth/{provider}/callback | provider별 code/state 거래 검증·서버 token 교환; 세션 회전/닉네임 진입; no-store |
| GET /api/v1/me | 인증 필요, 내부 account ID와 게임용 nickname만, no-store |
| PATCH /api/v1/me | 닉네임/약관 버전 검증, CSRF·Origin; 새 인증 후 onboarding 완료 |
| POST /api/v1/auth/logout | 서비스 세션 철회, 제공자 전체 로그아웃과 구분 |
| POST /api/v1/me/identities/{provider}/start | 최근 재인증 필요, account에 바인딩한 link 거래 |
| DELETE /api/v1/me/identities/{provider} | 최근 재인증·CSRF, 마지막 로그인 수단은 해제 불가 |
| POST /api/v1/me/export | 최근 재인증·CSRF, 본인 최소 데이터, token/subject 제외 |
| DELETE /api/v1/me | 최근 재인증·CSRF, 전 세션 철회·개인정보 삭제·제공자 token 철회 |
| POST /api/v1/auth/apple/notifications | Apple 서버 알림 서명/iss/aud 검증, 사용자 브라우저 API와 별도 |
| GET /api/v1/bootstrap | 지원 locale·공개 rules·daily versions·server availability, 정답/시드 없음 |
| GET /api/v1/daily/{date} | UTC date·공개 seed/solver version·rules hash, 캐시 가능 |
| POST /api/v1/daily/{id}/attempts | attempt_id, input_log, elapsed 주장, version; replay 결과와 verification status |
| GET /api/v1/daily/{id}/leaderboard | 페이지 제한, verified/local 구분, 닉네임 안전 렌더 |
| GET /health/live, /health/ready | live 프로세스, ready DB·큐·capacity 상태, 내부 상세는 공개 금지 |

변경 HTTP는 same-origin·CSRF 토큰 검사, 크기/레이트 제한을 적용한다. WS는 쿠키 인증과 Origin 검증, account별 한 active session epoch로 탈취·다중 탭을 제어한다.

## WS envelope와 상태 노출

```json
{"v":1,"type":"open","command_id":"UUID","match_id":"UUID","client_seq":12,"known_revision":44,"payload":{"cell":18}}
```

서버 응답: v, type, server_seq, revision, command_id(응답 연결), server_time_ms, payload. 클라이언트 시간은 승패에 사용하지 않는다. 단일 프레임 최대8KiB 제안. 숫자 cell은 0..255, UUID/enum/길이/version을 경계에서 검증한다.

server_seq/revision은 본인의 공개 stream 기준이다. 상대 flag·상대에게 보이지 않는 gauge/공격 등록만 변경됐을 때 본인의 revision을 증가시키거나 빈 delta를 보내지 않는다. 내부 전역 ingress sequence·analysis revision을 public 카운터로 내보내 공격 시점/개수를 추측하게 만들지 않는다.

| 방향 | type | 주요 payload |
|---|---|---|
| C→S | queue_join / queue_cancel | difficulty / queue_id |
| C→S | room_create / room_join / room_leave / ready | code, room_id |
| C→S | open / flag / accuse / attack | cell 또는 빈 payload, attack에 target 금지 |
| C→S | resume | match_id, last_server_seq, session_epoch |
| S→C | queued / room_state / matched | queue deadline, room seats, opponent_type |
| S→C | snapshot / delta | 본인 opened 숫자·mine·flag·gauge·stun, 상대 진행률·공개 기절, 남은 시간 |
| S→C | ack / error | applied/duplicate/rejected, 안정된 error code |
| S→C | match_end | win/loss/draw/abort, reason, safe opened, 오류·지목 집계 |

상대 열린 칸·숫자, 숨은 seed·지뢰, active lie 표시·target·source·truth는 보내지 않는다. command_id별 최근 응답을 매치 기간+유예 동안 보관해 재전송을 같은 결과로 돌려준다. client_seq 재사용/역행과 epoch 불일치 거절; revision gap은 snapshot으로 복구한다. 구버전은 unsupported_version 응답 후 재로드 안내한다.

`known-neighborhood-v1`의 재접속 snapshot에는 본인의 공개 숫자 변경 이력을 포함한다. 최초 zero opening과 이후 변경 배치만 전달하고 상대 이력·solver의 safe/mine 결론·등록 후보·논리 truth·lie flag를 넣지 않는다. 이력은 서버가 만든 최대512개의 비어 있지 않은 배치이며 중복 셀·범위 밖 셀·닫힘 복귀·변경 없는 배치는 거절한다. 클라이언트가 제출한 이력으로 온라인 상태를 교체하지 않는다. [ADR0010](../adr/0010-public-certified-attack-strategy.md)

## 빠른 매칭과 친구 방

```mermaid
sequenceDiagram
  participant A as Guest A
  participant Q as Matchmaker
  participant B as Guest B or Bot
  A->>Q: queue_join
  Q-->>A: queued deadline 10s
  alt human reserved before deadline
    B->>Q: queue_join
    Q-->>A: matched human
  else deadline reached
    Q->>B: create observable bot
    Q-->>A: matched bot difficulty
  end
  Q-->>A: snapshot countdown
  Q-->>B: snapshot countdown
```

```mermaid
sequenceDiagram
  participant A as Host
  participant R as RoomService
  participant B as Friend
  A->>R: room_create
  R-->>A: expiring code and link
  B->>R: room_join code
  R-->>B: seat reserved
  A->>R: ready
  B->>R: ready
  R-->>A: countdown
  R-->>B: countdown
```

인간 상대 예약과 봇 생성은 한 번만 일어나는 원자적 상태 전이로 구현한다. 기한 이상 수신된 인간 상대는 다음 큐로 들어간다. 방 코드는 암호학적 난수8문자 제안, 10분 만료, 열거 레이트 제한, seat 소유는 세션에 묶는다. 링크의 코드는 초대 기능만 제공하고 기존 플레이어 신원을 대체하지 않는다.

## 대전·공격·재접속

```mermaid
sequenceDiagram
  participant A as Attacker
  participant M as MatchActor
  participant V as LieValidator
  participant B as Defender
  A->>M: open command_id
  M-->>A: ack and public delta
  A->>M: attack command_id
  M->>V: candidate with revision
  V-->>M: valid certificate or rejection
  M-->>A: ack gauge or error
  B->>M: open corrupted cell
  M-->>B: displayed number only
  B->>M: accuse cell
  M-->>B: restored public number
  M-->>A: gauge zero and stun
```

```mermaid
sequenceDiagram
  participant C as Client
  participant M as MatchActor
  C->>M: socket lost
  Note over M: deadline keeps running, grace 30s
  C->>M: resume with cookie and last_server_seq
  M-->>C: new epoch and authoritative snapshot
  C->>M: retry old command_id
  M-->>C: cached ack without second transition
```

heartbeat는 15초 ping, 45초 무응답 시 끊김 판정을 제안한다. 실제 끊김 감지는 최대 이 지연이 더해질 수 있으며 reconnect grace는 서버가 감지한 disconnect 시점부터다. reconnect backoff 0.5/1/2/4/8초+지터, 30초 유예는 서버 권위로 표시한다.

## 데일리 제출과 한계

```mermaid
sequenceDiagram
  participant C as Cached Client
  participant D as DailyService
  participant R as ReplayVerifier
  participant DB as PostgreSQL
  C->>C: store input log with attempt_id
  C->>D: submit versioned attempt after recovery
  D->>R: replay public seed and input log
  R-->>D: valid rules and completion or rejection
  D->>DB: first valid result per account and day
  DB-->>D: idempotent result
  D-->>C: verified completion with timing class
```

서버가 start ticket·heartbeat로 측정한 online attempt만 verified_time 순위에 들어간다. 오프라인 replay는 verified_completion으로 **완료/오류 순위**에 별도 표기하며 클라이언트 시간은 개인 참고값이다. 입력 replay는 규칙 일관성을 검증하지만 수동 풀이·실제 오프라인 경과시간을 증명하지 못한다. 첫 유효 완료 한 건만 공식 기록, 이후 재시도는 연습. 구버전 replay verifier는 최소7일 보존 제안, 만료 기록은 개인 로컬 기록으로 유지한다.

## OAuth 경계

[17 인증](17-auth-privacy.md)을 따른다. Apple은 name/email scope 없는 code/query callback을 사용하고 form_post 추가는 별도 cookie 계약을 요구한다. callback은 일반 CSRF 헤더 대신 거래 검증을 수행한다. nickname 미설정 세션은 onboarding/me/logout/delete만 허용하고 대전·공식 기록에는 사용할 수 없다. 공식 attempt와 account 소유권을 검증하며 무계정 로컬 기록은 로그인 뒤 명시적 제출만 허용한다.
