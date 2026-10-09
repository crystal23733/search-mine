# ADR0033: 빠른 대전과 실제 온라인 보드·결과 화면

- 상태: 채택 — 승인된 #17/#70 구현 범위
- 날짜: 2026-10-09
- 선행: #68/PR69 병합·종료, develop4686ceb4

## 결정

Rust에서 생성한 LobbyResponse/OnlineEvent/GameView를 닫힌 런타임 디코더로 검증한다. 추가 필드·잘못된 버전/UUID/숫자/배열/variant는 거절하고 알려진 공개 필드만 새 객체에 담는다. 규칙·정답·공정성·봇·승패는 Rust가 판정한다. 출력 프레임 상한도 Rust 프로토콜 원천에서 TS로 생성한다.

로비는 고정 same-origin POST /api/v1/lobby, no-store/redirect error, bounded JSON과 #68 AuthenticatedRequests를 사용한다. 예상 account/revision과 전/후 fresh bootstrap을 확인한다. 대기 중 한 조회가 끝난 뒤 500ms 후 다음 조회를 시작한다. 취소는 서버가 반환한 정확한 queue/room/preparing identity를 사용하고 generation/abort로 늦은 응답을 차단한다. 이미 서버에 반영된 매치를 취소 응답 없이 취소 성공으로 표시하지 않는다.

WS URL은 같은 HTTPS origin의 /api/v1/ws이며 query·subprotocol·계정·match ID·credential을 넣지 않는다. 브라우저 cookie/Origin으로 인증하고 최초 snapshot을 배정 match와 연결한다. 전/후 bootstrap이 수락되기 전 이벤트 버퍼는 최대16개다. binary/초과 크기/알 수 없는 DTO/다른 match는 연결을 닫는다. 서버 sequence와 revision을 확인하고 snapshot/delta의 공개 view만 적용한다. 입력에는 UUID command ID·단조 client sequence·서버 epoch·known revision·PublicAction만 넣는다. ACK는 대기 command와 연결하며 대기 입력은 최대16개/10초다. optimistic 게임 규칙은 없다. gap/연결 종료에는 입력을 중단하고 오류/명시적 재시도를 제공한다. 자동 재접속·명령 재전송·30초 클라이언트 출구는 #18이다.

/queue는 하나의 controller 수명으로 3난이도 빠른 대기→준비→서버 countdown→실제 보드→결과/기록 상태를 표시한다. deadline과 server time에 브라우저 단조 경과 시간을 더한 값은 표시용이며 승패/봇 예약을 결정하지 않는다. 상대 DTO에 닉네임이 없으므로 사람/봇 표지만 쓰고 본인 닉네임은 현재 최소 계정에서 표시한다. Board/MatchSummary/MatchLayout와 디자인 토큰을 재사용하며 PC/mobile·키보드/touch·8언어를 지원한다. 언어 변경은 controller를 재생성하지 않는다.

무계정/닉네임 없음/인증 장애에는 번역된 로그인·등록·로컬 연습 출구를 표시한다. 대기부터 결과까지 activity lease를 유지하고 화면 종료에 해제한다. 계정/generation 변경과 dispose는 요청·WS를 취소하고 이전 공개 view를 지운다. online view/replay/credential을 일반 저장소에 기록하지 않으며 오프라인에서 로컬 권위로 전환하지 않는다. 기록 pending/saved/failed는 서버 상태 그대로 표시하고 pending 연결 종료를 saved로 바꾸지 않는다.

## 검증과 제한

디자인10의 결과 우선순위를 따른다. 종료 뒤 같은 controller/activity 수명 안에서 승패·이유·양측 공개 진행률·본인 통계·실제 기록 상태·새 상대/홈을 먼저 표시한다. 종료 보드는 접힌 상세 보기에서 읽기 전용으로 열며 없는 공격/반사 통계를 만들어내지 않는다.

페이지를 다시 열어 기존 대기를 복원하면 서버가 반환한 실제 난이도를 선택 표시에 사용한다. 배정된 봇에 난이도 정보가 없을 때 임의의 기본값을 실제 난이도로 표시하지 않는다. 선택 제안과 수락된 예약을 구분한다.

연결 종료 직후 입력을 중단한 뒤 같은 예상 권한에 한 번의 bounded bootstrap 검증을 실행한다. 서버 철회가 close frame으로만 전달돼도 이전 보드를 지우고, 정상 권한의 네트워크 단절은 수동 상태 확인으로 안내한다. 이 검증은 자동 재접속/입력 재전송을 시작하지 않는다.

디코더/HTTP/WS/controller/UI의 의미 있는 행동을 Red→Green으로 확인한다. 실제 Rust router/PostgreSQL/WS/HTTPS 브라우저 출구를 연결한다. loopback 테스트 fixture만 제공자 proof·통제 Clock·3×3/지뢰2/공격 게이지1의 작은 검증 판을 사용하고 실제 BoardPool/CoreMatchPreparer/CoreBot/MatchRegistry/PgResultRepository/합성 철회를 조립한다. HTTPS proxy는 고정 loopback WS upgrade를 전달한다. 공유 통제 Clock을 쓰는 브라우저 검사는 같은 serial suite에서 실행한다. 운영 router/인증/Clock을 우회하지 않는다.

두 계정의 동일 공개 초기 판·실제 입력/결과·10초 봇 3난이도·취소/계정 철회·8언어·광고 요청/온라인 저장0을 검증한다. 작은 fixture는 운영16×16/지뢰40 성능이나 공정성 실험을 대체하지 않는다. 실제 키 #42, 하드웨어 #26, 사람 관찰 #27은 별도다. 친구 방 UI는 다음 #17 하위 작업이며 상위 #17/#18을 이 작업만으로 종료하지 않는다.
