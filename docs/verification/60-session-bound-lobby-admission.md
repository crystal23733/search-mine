# #60 검증: 세션 수명과 원자 로비 입장

2026-10-09 · #17/M3 · [ADR0028](../adr/0028-session-bound-lobby-admission.md) · FR01/02/06/07/14/16, NFR02 · TS10/12/20/21/31~36. 선행 #58/PR59 develop0a9eb49 병합·종료.

## 구현과 실제 Red→Green

로비와 socket의 AuthorityRegistry를 분리한다. bind_shared는 동일 session hash/만료의 요청에 Weak/Arc identity를 공유하고 마지막 lease Drop에 반환한다. 두 인간의 원자 guard는 같은 lock 아래 identity/namespace/만료를 검사하고 pure registry 생성까지 보호한다. CombinedSessionInvalidator가 두 namespace의 철회 barrier를 DB 작업 전에 보유한다.

`lobby_authority.rs`의5개 실제 Red0passed/5failed→Green5와 기존 `online_authority.rs`4개 회귀를 실행했다. 공유 polling/Drop/capacity·세션 교체와 늦은 Drop·foreign/중복/빈 참여자·정확 expiry·합성 session/account 철회·실제 thread에서 guard 중 철회가 진입하지 못하는 경우를 검사했다. 최초 컴파일 단계의 Receiver Send 오류는 소유권을 수정한 뒤 행동 실패를 별도로 확인했다.

LobbyService는 account UUID 대신 검증된 lease를 받는다. 살아 있는 membership에만 lease를 보유하고 ticker/완료 처리 전에 철회·만료를 기존 cancel 정책에 연결한다. 입장/status와 최종 두 참여자 생성 시점에서 권한을 검사한다. 기존15개+추가5개의 실제 Red15passed/5failed→Green20(2.04s)를 실행했다. 실제 blocking CPU 중 철회/정확 expiry·원래 FIFO/기한 복구·ready host 만료/guest 승격·새 generation과 과거 완료 제거·모든 API의 foreign/revoked/expired 거절·요청 Drop 사이 membership 소유와 cancel 반환·socket 종료 독립성을 확인했다.

## 통합 검증의 상태

리뷰에서 별도 namespace가 조립 실수로 같아질 수 있는 경로를 추가했다. 같은 registry 생성 인자의 실제 Red0/1 뒤 생성 시 pointer identity를 검사하도록 수정했다. 최종 추가 테스트·전체 검사와 CI 결과를 PR에서 확인한다.

PostgreSQL 테스트는 logout/revoke_account/erase_authorized의 실제 SQL 삭제를 행 잠금으로 막아, DB에 세션이 남은 동안 두 namespace의 권한과 stale/pending handshake가 먼저 거절되는지 확인한다. 계정 삭제·다른 세션 범위·유효 다른 계정 보존도 검사한다. 로컬 실행 환경/CI 결과와 커버리지는 최종 PR의 최신 head 로그로 기록하며 아직 실DB 성공이라고 주장하지 않는다.

공개 HTTP Origin/CSRF/열거 제한과 main·배정 후 최초 WS 연결 수명·OAuth invite·8언어 화면은 다음 하위 작업이다. 현재 공개 lobby route는 없고 상위17은 OPEN이다. 키42·미니PC26·사람27의 실제 출구를 대체하지 않는다. 로컬 코드 리뷰는 ignored `review/YYYY-MM-DD.md`에 기록한다.
