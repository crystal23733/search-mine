# ADR0024 · 원자 매칭·친구 방과 실행 연결

2026-10-09 · #17/M3 · FR06/07·TS10/12 · 선행 #16 PR51/develop951dff11 병합·종료. 승인된 빠른 대전·친구 방을 검토 가능한 순서로 구현한다.

## 정책과 실행의 경계

#52는 IO 없는 LobbyState의 FIFO 큐·방·비동기 시작 reservation을 구현한다. 이어지는 하위 작업은 검증된 비공개 보드 공급/공개 관측 봇 actor, 인증된 큐/방 전송·OAuth 초대 거래/8언어 화면을 연결한다. 모든 하위 PR과 실제 통합 출구를 충족한 뒤 상위 #17을 닫고 #18로 진행한다. #52만으로 실제 온라인 매칭을 완료했다고 표현하지 않는다.

큐 기한은 서버 monotonic 입장 시각+10000ms다. 인간은 기한보다 이른 수신에서 FIFO로 예약한다. 난이도는 봇 fallback의 Easy/Normal/Hard이며 인간끼리는 같은 규칙의 공통 큐다. 기한 이상 도착한 사람은 이미 기한이 된 상대의 봇 예약을 바꾸지 않고 새 큐에 들어간다. 서버 clock 역행·overflow는 거절한다. 동일 queue/account의 재요청은 중복 큐를 만들지 않는다.

account는 큐/방/시작 준비 중 한 위치만 가진다. 시작은 account 두 개 또는 인간+명시적 bot의 reservation으로 예약하고, 큐/방 정책은 Board·SQL·socket을 소유하지 않는다. bounded 비동기 보드 준비는 reservation의 entity UUID와 단조 generation을 캡처한다. 취소/만료/준비 취소 뒤 이전 완료는 commit할 수 없다. 인간 예약 뒤 보드 준비 시간이 큐10초를 넘겨도 이미 결정된 인간 상대를 봇으로 바꾸지 않는다. 준비는 별도 최대5초이며 친구 방의 원래 만료보다 길 수 없다.

caller는 Lobby lock 아래 살아 있는 reservation을 확인하고 MatchRegistry 생성 성공과 정책 commit을 직렬화한다. 생성 실패는 reservation을 실패로 종료하고 사용자에게 unavailable을 표시한다. seed/정답은 전달하지 않는다. cancellation은 actor countdown 시작 전만 허용한다. 시작 후 규칙은 RuleEngine과 #18 재접속 계약이 결정한다.

## 친구 방

코드는 `ABCDEFGHJKLMNPQRSTUVWXYZ23456789`의 대문자8문자다. 암호학적 난수 source는 adapter가 제공하고 도메인은 canonical code와 live collision을 검사한다.10분 만료·최대2seat·account ownership·두 ready를 모두 요구한다. 같은 방의 재입장은 idempotent이며 세번째 계정/다른 큐·방에서의 중복 점유는 거절한다. code는 초대만 제공하며 인증/seat credential이 아니다. host가 나가면 남은 사람을 seat0으로 승격하고 준비를 초기화한다.

준비 중 unready/leave는 reservation을 무효화한다. 살아 있는 방의 남은 seat는 원래 만료 안에서 계속 사용할 수 있다. 큐의 준비를 한 사람이 취소하면 상대는 원래 큐 기한으로 복귀하고, 다음 tick에서 필요하면 봇을 예약한다. 만료 시 해당 방/준비 reservation과 membership을 제거한다. capacity는 모든 큐·방·준비 인원을 포함하며 별도 상한을 주입한다.

## 후속 연결과 검증

실제 큐/방 입력은 서비스 세션/정확 Origin·CSRF 또는 인증 WS epoch로 검증하며 code 열거를 별도 제한한다. OAuth 거래에는 canonical invite만 넣고 허용된 friends 목적지에 바인딩한다. callback→nickname/tutorial→friends에 서버의 거래 값을 복원하고 외부 return URL이나 OAuth secret을 client storage에 보관하지 않는다. migration은 기존 checksum을 보존하고 별도 추가한다.

#52는9999/10000·준비 만료·취소/늦은 완료·준비 generation 재사용·code collision/2seat/ready/leave·capacity를 fake server clock으로 검증한다. 실제 검증된 보드/봇·DB/OAuth 왕복·TCP WS와 browser 흐름은 후속 이슈의 필수 출구다. 공정성·20ms·mini PC·실제 제공자 키/사람 관찰은 별도 실제 검증을 유지한다.
