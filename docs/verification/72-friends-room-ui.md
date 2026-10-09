# #72 친구 방·초대·준비·온라인 대전 검증

대응: FR01/02/06/07/08/10/11/14 · NFR01/02/05/06 · TS02/10/12/13/15/18~22/26/31/36 · [ADR0034](../adr/0034-friends-room-ui.md).

## 구현과 경계

OnlineEntry가 계정 게이트·동일 controller/activity·연결/공개 보드/결과를 공유하고 QueuePanel/FriendsPanel은 로비 표현만 맡는다. 친구 방은 실제 Rust8문자 코드·occupied/ready/own_seat·서버 만료를 표시한다. 입력은 ASCII 소문자/앞뒤 공백만 정리하며 잘못된 코드를 전송하지 않는다. 초대 query로 자동 참가하지 않고 로그인/닉네임/튜토리얼 왕복에 기존 서버 OAuth 거래를 사용한다. 공유 링크는 같은 origin·locale·code만 담는다.

create/join/ready는 #70의 전후 권한 proof와 HTTP를 사용한다. 현재 room ID·실제 own seat·정확한 취소 identity와 generation/abort로 오래된 poll/명령 결과를 폐기한다. 로컬 clock의0초는 상태를 바꾸지 않는다. 서버만 만료/준비/배정/시작을 결정한다. 상대 계정/닉네임/정답을 공개 DTO에 추가하지 않는다. 명시적 나가기는 실제 cancel 응답을 확인하며 단순 페이지 닫기를 취소 성공으로 표현하지 않는다.

## 실행 근거

방 명령/초대의 stub에서3failed/8passed와 UI에서2failed를 실제 실행했다. 구현 후 관련14개가3.04s에 통과했다. 이후 리뷰 probe를 포함한16개가3.04s에 통과했고 웹 전체135개(8.94s), lines1948/2239=87.00%를 확인했다. 최종 scripts/check.ps1은 exit0으로 locked fmt/Clippy/native/WASM·생성 types/fixture/tokens·웹 형식/lint/typecheck/135unit/build·실제 HTTPS 포함75browser(1.8m)·문서125/Mermaid34 실패0을 통과했다. CI18/최신 head·병합은 PR에 실제 완료 후 기록한다.

실제 Rust auth/lobby/actor/WS·PostgreSQL17.4·HTTPS의3개가18.4s에 통과했다. PC1440/mobile390 두 계정의 실제 clipboard 링크·초기 튜토리얼→OAuth Google/Apple→nickname/tutorial→명시 join·third full·8언어 방 유지/레이아웃·ready/unready·동일 공개 판/WS 입력/실제 결과 저장과 친구 로비 복귀를 검사한다. 호스트 이탈의 seat 승격/준비 초기화, 실제 서버601초 만료·old code not_found·잘못된 코드 미전송·실제429/열거 제한도 확인한다. 공유 URL의 credential 제거·공개 WS secret 없음·광고 요청/온라인 저장0을 검사했다.

최초 HTTPS 시도는 Browser 타입과 알려진 번역 locator를 정리하기 위해 중단했다. 다음 실행은2passed/1failed(45.6s)였으며 trace의 실제 버튼은 Check match status인데 test가 Check status again을 기다린 것이 원인이었다. 제품/timeout/retry를 바꾸지 않고 locator만 수정해3개가 통과했다. 초기 lint의 status 태그2개는 output으로 수정했다. 첫 전체 게이트는 추가 번역 키의 JSON 형식에서 중단됐으며 Prettier 정리 후 전체 게이트를 통과했다. 실패와 trace는 로컬 review/.tmp에 보존한다.

fixture의 작은3×3/지뢰2·통제 Clock·외부 provider proof는 #70과 같고 실제 제품 adapter/저장/철회는 유지한다. 운영 PostgreSQL18 CI와16×16/글로벌 성능·실계정 검수를 대체하지 않는다.

## 코드 리뷰

pm-ai-shipping code-review의 correctness를 scope72/base37914fd5에 순차 적용한다. 요청 초대A≠기존 서버 방B·own seat 재배정, poll→ready/cancel/late response·중복 클릭/계정 교체, 공유A→나가기→방B/공유B→역순 완료와 표시용 시간0/실제 상태를 강제 probe로 실행했다.

P2: 새 방의 복사가 빠르게 완료되면 방 변경 effect가 완료 안내를 다시 지우는 실행을1failed/15passed로 재현했다. FriendsPanel의 공유 완료와 방 변경 effect 사이의 수명 불일치였다. 단순250ms 재렌더는 사라진 안내를 복원하지 못한다. ADR을 먼저 갱신하고 안내 자체를 room ID에 묶으며 최신 공유 순서·현재 방·언마운트를 확인하도록 수정했다. 역순 이전 방 실패가 새 방 성공을 덮지 못하는 검사까지 통과했다. Auth invalidation의 activity 해제는 DOM 교체와 다른 effect 경계이므로 기존 계약대로 실제 busy=false를 기다린다.

공개 입력→canonical code→HTTP, 서버 room/seat/ready→UI, query→로그인 거래/공유 URL, controller/리스→재사용 Board/Result와 오류 경계를 함께 읽었다. 검토한 범위의 추가 근거 있는 결함은 없었다. 전체 성능 감사나 실계정/사람 검수를 수행했다는 의미는 아니다. 자동 재접속/명령 재전송/30초 유예 클라이언트 출구는 #18이다.

## CI 철회 경합의 추가 수정

최초 CI Product37897139375의 웹 검사는74passed/1flaky(재시도 성공)였다. 기존 빠른 대전의 logout204 뒤5초에도9cell이 남았다. PostgreSQL18.6 DB26개(ignored0/3.64s, coverage 재실행7.31s)와 다른 검사는 성공했지만 병합을 중단했다. PgAccountRepository.logout의 철회 barrier가 await DELETE보다 먼저 WS를 닫으므로, 직후 bootstrap이 아직 유효한 DB 세션을 읽는 경합이 있었다.

ADR0033을 먼저 갱신하고 선행 공개 오류가 없는 정책 close1008은 로컬 권한을 즉시 폐기하도록 변경했다. 서버도 outgoing/watch 종료 경합에서 권한이 없으면1008을 보낸다. 일반1000/1006 네트워크 종료의 bounded proof와 결과 pending 보존, 선행 rate/malformed 오류는 유지한다. 웹2failed/14passed Red 후 관련17개와 실제 TCP WS10개가 통과했다. 새 head의 최종 전체/HTTPS/CI 결과는 PR에 기록한다.

수정 뒤 단일 worker 실제 HTTPS 철회6회는 재시도 없이6passed(23.0s)였다. repeat 간 병렬 실행은 공유 fixture 시계/큐를 서로 진행시켜6failed였으므로 trace를 보존하고 단일 worker로 실행했다. 마지막 scripts/check.ps1 exit0:138unit·lines1952/2242=87.06%, 실제 HTTPS 포함75browser(1.9m), locked fmt/Clippy/native/WASM·생성 freshness·형식/lint/typecheck/build·docs125/Mermaid34 실패0. 새 head의 CI는 PR에서 별도 확인한다.
