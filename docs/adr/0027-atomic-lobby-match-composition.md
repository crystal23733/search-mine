# ADR0027 · 준비와 시작을 나누는 원자 매치 조립

2026-10-09 · #58/#17/M3 · FR02/06/07·TS02/10/11/12/21 · 선행 #56 PR57/develop8dc226b 병합·종료. [ADR0024](0024-atomic-lobby-and-online-composition.md)·[보드 풀](0025-bounded-private-certified-board-pool.md)·[온라인 봇](0026-public-observation-online-bot-driver.md)을 내부 application service로 조립한다.

core PreparedEngine은 private Board/rules/인증된 초기 player 상태만 소유하며 Debug/Serialize/Clone/advance/projection이 없다. 무거운 인증·초기 관측 준비를 끝낸 뒤 start(now)가 새 RuleEngine을 소비 생성한다. countdown/duration은 그 server now에서 시작한다. 기존 new/new_solo와 native/WASM의 유효 입력 규칙·재생 계약을 유지하며 시간 계산은 Rust core 하나다.

LobbyService의 clock은 MatchRegistry가 소유한 같은 MatchClock이다. Mutex 아래 한 시각을 캡처해 policy tick→live reservation 확인→prepared engine 시작→registry 생성 성공→policy commit을 직렬화한다. 실패면 해당 reservation을 끝내고 참여 account의 unavailable/capacity를 제한된30초 오류 상태로 남긴다. 현재 match/live membership이 오류보다 우선하며 새 성공 입력은 해당 account의 이전 오류를 지운다. 오류 map도 lobby capacity를 넘지 않고 오래된 항목을 정리한다. core 준비·SQL·await는 Lobby lock 안에서 실행하지 않는다.

작업 key는 entity UUID+단조 generation이며 oneshot receiver는 그 key의 job이 소유한다. 준비 slot1~64는 BoardSource take의 최대2초 대기와 실제 MatchPreparer blocking 작업을 포함한다. slot은 closure로 이동해 실제 CPU 종료까지 유지한다. cancellation/expiry/service Drop은 waiting take를 중지하고 receiver를 제거하며 시작한 CPU를 중지했다고 가정하지 않는다. old completion은 새 generation을 찾아가거나 registry를 만들 수 없다. 준비는 FIFO reservation generation 순으로 admission하고20ms ticker는 Weak service만 보유한다.

입력은 account ownership으로 적용하며 queue 재요청은 기존 ticket ID를 재사용한다. cancel은 caller가 관측한 Queue/Room/Preparing identity와 현재 위치가 같을 때만 적용하고, 늦은 이전 cancel이 새 큐/방을 지우지 않게 한다. 이미 시작한 active match의 membership 변경은 Busy다. 친구 코드는 별도 RoomCodeSource의 OS entropy로 canonical8문자를 만들고 collision은 최대8번 재시도한다. 코드가 credential은 아니며 실제 session/Origin/CSRF·열거 admission은 후속 transport의 책임이다.

actual certified BoardSource/CoreMatchPreparer/MatchRegistry/core bot·public snapshot을 통합한다. 통제된 actual blocking 준비와 clock으로 cancel/unready/new generation·5초/10분 expiry·failure/registry capacity·Drop·same countdown timestamp·old cancel/late completion을 Red→Green으로 검증한다. 이 서비스는 아직 public HTTP/WS route/main/OAuth invite/UI가 아니며 상위17·외부키42·miniPC26·사람27의 완료를 주장하지 않는다.
