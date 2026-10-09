# ADR0034: 친구 방·초대·준비와 온라인 화면 재사용

- 상태: 채택 — 승인된 #17/#72·FR07 구현 범위
- 날짜: 2026-10-09
- 선행: #70/PR71 병합·종료, develop37914fd5

## 결정

디자인06의 방 만들기·코드 참가·코드/링크·두 자리·준비·나가기를 /friends에 연결한다. 그림의 예시6문자 대신 승인된 Rust 계약의8문자 코드를 사용한다. UI는 입력의 앞뒤 공백과 ASCII 소문자를 canonical 코드로 정리하고 잘못된 문자/길이는 전송하지 않는다. 초대 URL은 같은 origin의 현재8언어 /friends와 code만 담으며 계정·닉네임·인증/게임 정보를 포함하지 않는다. copy/native share는 기존 SharePort를 사용하고 실패/취소에 별도 안내를 제공한다. 자동 공유는 없다.

공유 결과 안내에는 시작한 room ID와 로컬 동작 순서를 연결한다. 방 전환/언마운트 또는 새 공유 뒤의 늦은 완료를 폐기한다. 방 변경 effect가 이미 완료된 새 방의 안내를 지우지 않도록 표시할 안내 자체를 방 identity에 연결한다.

초대 query는 참가 입력에 채운다. 로비를 조회한 뒤 사용자가 명시적으로 참가하며 자동 참가/방 변경은 없다. 무계정/닉네임 필요 화면은 friends 목적지와 유효 code를 로그인/닉네임으로 전달한다. #66의 서버 OAuth 거래→callback→nickname/tutorial 복귀를 재사용한다. 초대는 계정 세션을 대신하지 않고 브라우저 영속 저장을 추가하지 않는다.

OnlineController에 room_create/room_join/ready를 추가한다. 명령은 예상 account/revision·fresh proof와 기존 private HTTP를 사용한다. 한 번에 한 사용자 명령, 현재 room ID, 실제 own_seat의 ready, 정확한 room/preparing 취소 identity와 generation/abort를 적용한다. 명령을 시작할 때 이전 poll을 중단하며 늦은 응답이 새 방을 덮지 못한다. 낙관적인 seat/ready/성공·봇 전환은 없다. 둘이 준비되면 Rust가 준비/배정/시작을 결정하고 기존 private WS·공개 Board/Result를 사용한다.

로비의 occupied/ready 두 배열과 own_seat만 표시한다. 본인 nickname은 현재 AuthPort에서, 다른 사람은 번역된 상대 표지로 나타낸다. 상대 계정/사진/닉네임 공개 DTO를 추가하지 않는다. 호스트 이탈 후 seat 이동과 준비 초기화도 서버 응답 그대로 반영한다. 공개 expires_at_ms와 server_time_ms의 차이에서 표시용 시간을 줄이고, 만료/시작을 클라이언트 시각으로 결정하지 않는다. 실제 status가 idle이면 방 종료 안내와 새 create/join을 제공한다. full/not_found/rate_limited/busy는 구분하되 존재하지 않는 방과 만료한 방은 같은 안내를 사용한다. room code를 탐색하거나 자동 재시도하지 않는다.

OnlineEntry는 계정 게이트·controller/activity 수명과 오류/연결/보드/결과를 공유하고 QueuePanel/FriendsPanel은 로비 표현만 책임진다. 언어 변경에 controller를 재생성하지 않는다. 결과의 다시 대전은 서버 status를 새로 확인해 각 모드의 초기 화면으로 돌아간다. 명시적인 나가기는 서버 cancel의 idle 응답을 확인한다. 단순 Home/페이지 닫기는 로컬 연결/타이머/리스만 해제하며 성공한 서버 취소를 추측하지 않는다. 남은 방은 재방문 status에서 복원되고 실제 서버 만료를 따른다.

controller의 역순 poll/ready/cancel·현재 identity/own seat·중복 클릭/권한 변경을 Red→Green으로 확인한다. 실제 Rust/SQL/HTTPS/WS serial fixture에서 create→공유 링크→OAuth/nickname/tutorial→명시 join→full→ready/unready→동일 판/결과, 호스트 이탈/승격·만료·잘못된 코드·8언어 PC/mobile과 광고/온라인 secret 저장0을 검사한다. 새 의존성이나 규칙 복제는 없다.

실제 OAuth 키 #42·하드웨어/글로벌 RTT #26·사람 #27, 자동 재접속/명령 재전송·30초 유예 클라이언트 출구 #18은 별도다. #17은 이 작업의 실제 통합 결과와 병합을 확인한 뒤 닫는다.
