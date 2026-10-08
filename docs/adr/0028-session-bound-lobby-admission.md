# ADR0028: 로비의 세션 수명과 원자 입장 권한

2026-10-09 · #60/M3 · 상위 #17 · FR01/02/06/07/14/16, NFR02 · TS10/12/20/21/31~36. 선행 #58/PR59 develop0a9eb49 병합·종료.

## 결정

공개 전송 전에 LobbyService의 account UUID 입력을 검증된 ConnectionAuthority 입력으로 교체한다. LobbyAuthentication은 별도 AuthorityRegistry와 AuthClock을 주입한다. 정책 도메인은 인증·DB를 모른다. 서비스는 살아 있는 membership에만 lease를 보유하며 보드/CPU 준비 중에도 서버가 검증한 세션의 철회·만료를 적용한다. status를 포함한 모든 외부 호출과 registry 생성 시점에 권한을 다시 검사한다.

게임 socket과 로비는 별도 authority namespace다. 같은 세션 hash/만료의 로비 요청은 bind_shared로 같은 token/Arc identity를 공유한다. 짧은 HTTP 요청이 끝나거나 polling을 반복해도 대기열의 lease가 끊기지 않는다. 다른 세션의 재입장은 기존 로비 권한을 교체하고 과거 요청/Drop이 새 권한을 지우지 못한다. 기존 socket bind의 마지막 연결 우선 동작은 유지한다. 공유 identity는 Weak로 저장하여 namespace가 lease의 수명을 연장하지 않는다.

LobbyService 생성 시 socket과 같은 registry를 주입하면 거절한다. 단순 조립 실수로 HTTP polling과 WS의 마지막 연결 우선 정책을 섞지 않는다. transport는 저장소 session을 읽기 전에 generation을 캡처하고 검증된 hash/account/expiry만 bind_shared에 보낸다.

CombinedSessionInvalidator는 세션 hash 또는 계정의 철회 barrier를 두 namespace에 동기적으로 적용한다. persistent logout/삭제/연결 변경보다 먼저 시작하고 DB 작업이 끝날 때 두 barrier를 반환한다. 철회 전/중에 읽은 session으로 뒤늦게 bind할 수 없다. HTTP/main 조립은 다음 하위 작업에서 이 합성 invalidator를 실제 PgAuthStore에 주입한다. #60의 실DB 검사로 저장소와 이 계약을 미리 확인한다.

잠금 순서는 Lobby→로비 authority→match registry이고 역방향 callback은 없다. 로비의 ticker는 policy tick/완료 처리 전에 철회·만료된 membership을 취소한다. 유효 상대의 원래 FIFO 순서/기한과 살아 있는 친구 방을 보존한다. 최종 두 인간 lease의 검증과 registry 생성은 같은 authority lock 아래 수행한다. 이 임계 영역에 IO/await/보드 인증/봇 계산을 넣지 않는다. 검사와 생성 사이 철회가 생긴 경우 생성하지 않고 기존 취소 정책으로 상대를 복구한다. 취소된 key의 실제 CPU는 기존 worker permit을 끝날 때까지 보유하고 늦은 완료는 버린다.

성공·실패·cancel·만료 뒤 inactive membership lease를 제거한다. 이 메모리는 lobby capacity 안에 제한된다. 봇은 서비스 계정/lease를 만들지 않는다. 공개 projection/DTO에는 session hash/token/계정 UUID/Board/seed를 넣지 않는다.

## 검증과 남은 범위

실제 Red→Green으로 shared lease/최종 소유자 Drop/교체·foreign namespace·두 사람 원자 guard·합성 철회 barrier를 확인한다. 로비는 blocked 실제 CPU 중 logout/account revoke/정확 만료, peer 복구와 새 generation, 만료된 host 승격/room ready 취소, 늦은 이전 lease/Drop, active socket과 독립성을 검증한다. PostgreSQL CI에서 두 namespace를 가진 실제 저장소의 logout·계정 철회를 검사한다.

공개 HTTP의 Origin/CSRF/열거 제한과 main, 매치 배정 이후 최초 WS 연결 수명, OAuth 초대 거래·8언어 UI는 다음 #17 하위 작업이다. 초기 연결이 없는 매치를 완료했다고 주장하지 않으며 현재 공개 lobby route는 없다. #42 실제 제공자 키·#26 미니 PC·#27 사람 관찰은 별도 실제 출구다.
