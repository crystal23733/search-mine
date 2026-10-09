# ADR0036: 상대 연결 유예의 서버 권위 공개 상태

- 상태: 채택 — 승인된 FR14/#18의 연결 상태 구현
- 날짜: 2026-10-09
- 선행: #74·PR75 병합·종료, develope27f9226; 구현 #76

## 결정

현재 RuleEngine의 disconnect 시각과 고정된 rules.reconnect_grace_ms에서 공개 opponent.reconnect_ms:Option<u32>를 투영한다. null은 연결됨 또는 종료된 매치, 양수는 서버가 감지한 끊김 이후 남은 유예,0은 해당 상대의 유예가 지났으나 권위 결과를 기다리는 상태다. 본인 연결 상실 시각이나 브라우저 타이머로 서버의 감지 시각을 추정하지 않는다. 서로 다른 시각에 두 명이 끊겼다면 먼저 끊긴 사람의 유예가0인 동안에도 전체 abandon은 아직 확정되지 않을 수 있다.

기존 Rust의30초/forfeit/abandon/timeout 우선순위·정확 경계·resume epoch/cache 규칙을 바꾸지 않는다. 종료 후 유예 표시를 지우며 Rust가 제공한 result만 승패/중단의 원천이다. 공개 상대 정보에 연결 유예만 추가하고 보드/숫자/flag/gauge/cursor/계정/secret은 추가하지 않는다. 로컬/WASM 대전의 같은 GameView는 연결 유예가null이다.

MatchState의 Visible 비교는 양수 유예를 절대 deadline으로 정규화해 잔여 감소만으로 revision·빈 delta를 매tick 늘리지 않는다. null/양수 deadline/기한 만료0의 전환은 구분한다.0을현재시각+0으로 정규화해 tick마다 달라지는 버그를 만들지 않는다. 끊김과 복귀는 실제 공개 상태 전환이고 늦게 닫힌 이전 epoch는 새 연결을 끊을 수 없다.

Rust projection→protocol DTO→생성 TS·native/WASM fixture와 웹 strict decoder를 함께 갱신한다. 배포 전v1의 동시 변경이며 구 decoder가 새 필드를 수락한다고 주장하지 않는다. schema는 필수 null/u32이며 생략/음수/소수/범위 초과/추가 상대 필드는 fail closed 한다. reconnect_ms는 해당 rules의 reconnect_grace_ms를 넘을 수 없다.

웹 controller는 받은 잔여에서 로컬 단조 시간 경과분을 빼 표시만 갱신한다.0에서 상태를 connected나forfeit로 바꾸지 않고 서버 결과 대기 문구를 유지한다. 공통 ReconnectNotice는 디자인15의 상태 메시지를 참고해8언어 텍스트와 progress를 표시한다. 실제 모바일 첫 화면에서 MatchSummary 안의 안내가 보드 아래에 가려진 것을 확인했으므로 OnlineEntry에서 보드보다 먼저 배치한다. 연결된 playing 상태에서만 상대 안내를 표시한다. 양수는 상대 재연결 안내·남은 초·시간 계속 진행,0은 서버 결과 대기다. 본인의 온라인 입력은 연결된 상태에서 계속 가능하며 광고를 추가하지 않는다.

## 검증과 후속

코어/MatchState의 초기null·disconnect·중복disconnect·경과·resume·옛epochclose·정확기한·서로 다른 두 유예의0/abandon·종료 상태를 검증한다. 실제 actor/WS 및 PostgreSQL/HTTPS PC/mobile에서 상대 이탈→공개 유예→복귀/기한 초과 결과와 진행시간·공정성 경계를 확인한다. 웹은0의 결과 미확정·서버 결과 도착·연결 복귀/8언어/모바일 상태를 검증한다.

자동 backoff·메모리 미확인 명령 재전송과 영속 active match journal·프로세스 재시작 abort/결과 재조회는 후속 #18 하위 작업이다. 이 공개 유예 계약만으로 상위18을 종료하지 않는다. 실제 키42·미니 PC26·사람27 출구도 대신하지 않는다.
