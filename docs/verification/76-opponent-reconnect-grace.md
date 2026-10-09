# #76 상대 연결 유예의 권위 상태와 화면

대응: FR14/NFR01/02/05/06 · TS15/16/20/21/26 · [ADR0036](../adr/0036-opponent-reconnect-grace.md). #74/PR75/develope27f9226 병합·종료 후 시작했다.

## 구현과 실제 실행

Rust core의 상대 disconnect 시각과 고정 grace에서 reconnect_ms를 투영한다. null/양수/0을 구분하고 종료 후null이다. protocol→생성 TS·native/WASM fixture·strict decoder를 함께 갱신했다. Visible은 양수의 절대 deadline과 만료0을 정규화하여 시간 감소만으로 빈 delta를 내보내지 않는다. 웹은 수신 잔여의 로컬 표시만 줄이며0에서는 서버 결과 대기, 연결된 사용자 입력은 계속 허용한다. 공통 ReconnectNotice의8언어 output/progress와 디자인15의 상태 메시지를 사용한다. 실제 모바일 시각 검토에서 안내가 보드 밑에 가려져 ADR을 먼저 갱신하고 보드 앞에 옮겼다. DOM 순서1failed Red(2.83s) 후 검증한다.

Rust 공개 유예가Null인1failed Red(0.00s), 웹 decoder의1failed/5passed Red(19.68s), 웹 controller의5000→4000 미감소와 UI 누락2failed/14passed Red(3.99s)를 실행했다. Green은 core17개(0.39s)·protocol4개·state14개, 웹22개(2.88s)·타입/lint, actor5개(0.28s)·실제 TCP WS11개(0.07s)다. 실제 PostgreSQL17.4/Rust/HTTPS PC1440·mobile390의2개(18.5s)는 상대 이탈→본인 유예/8locale/가로 overflow0/입력 가능→같은 match/새 epoch 복귀/진행시간→두 번째 이탈 후 서버 forfeit·Win·실제 SQL Saved를 확인했다. 양쪽 이탈/서로 다른 유예0·abandon의 정확 경계는 native로 확인하며 실제 재시작/결과 재조회 출구를 대신하지 않는다.

기존 slow-consumer actor 시험은 퇴장한 상대의 새 공개 유예 delta가 error보다 먼저 도착하여 첫 실행4passed/1failed였다. 타임아웃을 바꾸지 않고 해당 delta의30000 값과 뒤따르는 정확한 Malformed error, 남은 seat 권한을 검증하도록 보정했다. UI lint의 role=status는 semantic output으로 수정했다. 게임 승패/타이밍 규칙과 의존성·스키마·제품 update timeout/retry는 바꾸지 않았다.

최종 scripts/check.ps1은 exit0이다. locked fmt/전체 target Clippy/native/WASM·생성 타입/fixture/tokens·웹 format/lint/typecheck/전체 unit/build·실제 HTTPS 포함79browser(2.0m)·문서129/Mermaid34오류0을 통과했다. 웹 line coverage는1959/2247=87.18%다. 별도로 실제 PostgreSQL17.4 DB26개(ignored0/2.87s)를 실행했다. 최종 화면 위치의 PC/mobile 실제 HTTPS 시험도 전체 검사에서 통과했고 두 화면을 시각 검토했다. 기본 native의 ignored DB는 실제 DB 통과로 세지 않는다. CI18의 실제 결과·최신 head/병합은 PR에 기록한다.

## 코드 리뷰와 제한

pm-ai-shipping code-review correctness를 scope76/basee27f9226에 순차 적용했다. 서버 감지 시각과 클라이언트 표시 시각을 다르게 만들어 표시0에도 playing/result없음/입력 가능을 확인했다. 같은 상대 유예의 양수→0과 두 사람 전체 abandon 시각도 다르게 만들었다. epoch 교체와 오래된 close, 공개 revision/절대 deadline/0 정규화, Rust DTO와 생성/strict decode/WASM/실제 HTTPS 생산·소비를 검토했다. native/WASM fixture의33개 상대 reconnect_ms:null 추가 외 의미 변경0을 파싱 비교했다. 실제 PC/mobile 시각 검토에서 보드 밑 안내를 발견해 ADR→DOM 순서 Red→공통 보드 앞 카드 Green(관련 웹22/2.97s)으로 보정했다. 검토 범위의 추가로 뒷받침되는 결함은 없었다. 별도 성능·보안 감사를 했다고 보고하지 않는다. 제품 seed/정답·상대 cursor/보드/계정은 추가하지 않는다. 자동 backoff/미확인 명령·영속 journal/재시작 abort/결과 조회는 후속18, 키42/장비26/사람27은 별도다.
