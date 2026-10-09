# #62 검증: 배정 후 최초 연결

2026-10-09 · #17/M3 · [ADR0029](../adr/0029-initial-match-connection-lifecycle.md) · FR02/06/07/14/16, NFR02 · TS10/12/15/16/20/21/35. 선행 #60/PR61 develop904de85 병합·종료.

## 구현과 실제 Red→Green

Rust core에 논리 시작 기한과 엄격한 시작 전 취소를 추가했다. core 행동3개 실제 Red0passed/3failed 뒤 Green3을 확인했다. 정확 기한의 취소와 completed=false·승자 없음, 역행/시작 후/종료 취소 거절, 후속 timeout이 결과를 바꾸지 못하는 경우를 검사했다. TS 규칙이나 공개 입력 권한은 추가하지 않았다.

create_admitted는 countdown·seat/account·human/bot 형태·별도 authority namespace를 확인하고 같은 로비 권한 lock에서 registry 배정과 actor의 초기 lease 소유권을 만든다. actor는 매 ingress 전에 남은 인간의 권한과 Rust 시작 기한을 검사한다. 첫 attach는 로비→게임 권한→core 순서이며 성공 reply/살아 있는 송신 sink 뒤 해당 초기 lease를 반환한다. 모두 처음 연결하면 기존 게임 authority/epoch/재접속 계약을 따른다.

actor 행동7개는 실제 Red1passed/6failed 뒤 Green7(0.09s)이었다. 늦은40초 tick의 논리3초 취소·첫 연결 없음·session/account 철회/정확 expiry·기한1ms 전의 두 인간/정상 입력·정확 기한 attach 거절·계정 없는 봇/미연결 인간·잘못된 seat/namespace와 철회된 배정 거절을 실행했다. 취소는 결과1개와 account 반환을 기존 저장/정리 경로로 처리한다.

실제 로비 전달의 별도 행동은 초기 lease 조기 반환으로 Red0/1이었다. LobbyService를 create_admitted에 연결하고 중복 authority lock을 제거했다. 실제 인간/봇 배정 뒤 요청을 모두 Drop해도 actor가 권한을 소유하고, 늦은 tick에서 취소/최종 Drop을 확인했다. 기존21개와 추가1개 총22Green(2.03s), 실제 core 봇 정상 fallback도 통과했다. 결과의 ended_ms는 배정 후 경과 시간이므로 봇의 절대 시작13000이 아니라3000을 검사한다.

## 통합 검사와 경계

실제 TCP WebSocket의 admitted fixture로 기한1ms 전 snapshot→미연결 상대의 cancelled/saved/completed=false와 정확 기한 handshake409를 검사한다. 공개 송신 DTO는 Serialize 전용을 유지하고 wire JSON을 관측한다. 전체 native/WASM·기존 WS/DB·web·docs·coverage의 최종 실행 결과는 최신 PR head와 로컬 review에 기록한다. 이 문서는 미실행 실DB나 목표 성능을 성공으로 계산하지 않는다.

pm-skills 코드 리뷰는 scope62/base904de85의 정확성·보안 흐름을 순차 검토했다. 늦은 receipt의 제안 시각과 core의 논리 종료 시각을 강제로 다르게 했으며, 철회된 원래 준비/새 generation의 완료, 잘못된 seat/namespace, 초기 lease Drop과 두 authority guard를 실제 시험과 대조했다. 같은 lock에 재진입하지 않고 마지막 lease Drop은 guard 밖이다. 새 session/Board/seed 직렬화나 공개 DTO 필드는 없다. 검토한 범위에서 추가로 뒷받침되는 결함은 남지 않았다.

공개 HTTP/main·OAuth 초대·8언어 화면은 다음 #17 하위 작업이다. 상위17은 아직 OPEN이며 실제 키42·미니PC26·사람27의 검증을 대체하지 않는다.
