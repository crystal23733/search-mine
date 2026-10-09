# #70 빠른 대전·실제 온라인 보드·결과 검증

대응: FR01~06/10/11/14/15, NFR01/02/05/06 · TS02/04~11/14~16/18~22/26/31/36 · [ADR0033](../adr/0033-online-quick-match-ui.md).

## 구현과 경계

Rust 원천 MAX_OUTPUT_BYTES를 TS로 생성하고 서버 WS 출력에도 사용한다. 웹의 closed decoder는 공개 로비/게임/이벤트를 새 객체로 구성하고 버전/UUID/정수/variant/길이와 중첩 비공개 필드를 거절한다. 로비는 fixed same-origin HTTP와 #68의 예상 권한/전후 proof를 사용한다. WS는 고정 cookie/Origin URL로 최초 snapshot을 배정 ID에 연결하고 전후 권한 확인 뒤 bounded 버퍼를 적용한다. CSRF·계정·seed·seat·브라우저 시각을 입력이나 URL에 추가하지 않는다.

controller는 대기 조회를 한 번에 하나씩 500ms 간격으로 실행하고 정확한 취소 identity와 generation/abort로 늦은 조회/연결을 폐기한다. 서버 sequence·epoch·revision·command UUID/ACK와 최대16개/10초 입력을 연결하며 optimistic 규칙은 없다. 서버 철회가 close frame으로만 전달돼도 한 번의 같은 권한 proof로 이전 보드를 지운다. 실패한 결과 기록을 저장 성공으로 바꾸지 않는다.

/queue는 세 난이도·서버10초 봇·취소·서버 countdown·Board/MatchSummary/MatchLayout·결과를 연결한다. 실제 대기 복원의 난이도는 서버 응답을 표시한다. 상대에게 없는 nickname/통계는 만들지 않는다. 언어 변경에 controller를 보존하고 결과까지 activity lease를 유지한다. 디자인10에 따라 결과·진행률·통계·기록을 먼저 표시하고 종료 보드는 읽기 전용 상세 보기로 제공한다. 무계정/닉네임/장애에는 번역된 로그인과 로컬 연습 출구를 제공한다.

## 실행 근거

DTO stub4·HTTP stub2·WS stub2·controller stub6·UI1에서 실제 행동 실패를 실행했다. WS 즉시 거절의 추가 Unhandled1과 DTO만 실행했을 때 전체 coverage 미달은 성공으로 계산하지 않았다. 구현 후 경계/UI18개가3.02s에 통과했고 웹 전체128개(9.13s), lines1863/2149=86.69%를 확인했다. 결과 화면을 포함한 최종 전체 검증에서도128개와 lines1872/2159=86.70%를 확인했다.

실제 Rust auth/lobby/WS·PostgreSQL17.4·HTTPS 브라우저6개가21.9s에 통과했다. 외부 provider proof와 통제 Clock·3×3/지뢰2/게이지1만 loopback fixture로 대체하며 BoardPool/CoreMatchPreparer/CoreBot/MatchRegistry/PgResultRepository/합성 철회는 실제 제품 어댑터다. 같은 serial suite에서 두 계정 PC/mobile의 동일 판, 깃발/열기, 해당 command ID의 공격 거절·수락 ACK, 거짓 숫자/간파와 반사 기절, timeout/실제 저장, 세 난이도10초 봇 진행, 취소,8언어 대기와 서버 logout을 검사한다. 공개 WS 입력의7개 필드·출력의 secret 없음·온라인 데이터 저장/광고 요청 없음도 확인한다.

리뷰의 실제 reload probe에서 기존 서버 Easy 대기를 기본 Normal 선택으로 표시하는 실패를 재현했다. 서버 난이도 복원 후 세 난이도의 reload를 포함한6개가 통과했다. close-only 철회 역시 실패 probe를 먼저 실행하고 bounded 권한 검증으로 수정했다. BOT 표시의 첫 실패는 progress 숫자가 포함된 문단 전체를 exact text로 찾은 test locator 오류였고, actual DOM의 BOT 표지는 유지했다. trace 원본과 이전 문자 인코딩/타입/lint 실패도 ignored review에 기록한다.

실제 PostgreSQL26개는 ignored0/실패0으로12.46s에 통과했다. 로컬 DB는 Docker 엔진이 응답하지 않아 작업공간 .tmp의 전용 PG17/loopback을 사용했다. 운영/CI의 PostgreSQL18 검사를 대신하지 않는다. 최종 scripts/check.ps1은 exit0으로 locked fmt/Clippy/native/WASM·생성 types/fixture/tokens·웹 형식/lint/typecheck/128unit/build·실제 HTTPS를 포함한72browser(1.6m)·문서123개/Mermaid34개를 통과했다. CI18·Rust95%·최신 checks/head와 병합은 PR에서 실제 완료 후 기록한다.

## 리뷰와 제한

최초 최종 게이트의 브라우저는68passed/2failed/2미실행이었다. 두 실패는 기존 cache URL 검사가 공개 local-session-*.js 파일을 인증 데이터로 분류한 것이다. [ADR0018](../adr/0018-public-offline-cache-and-safe-update.md)을 먼저 갱신해 같은 origin·실제 API/인증 경로 segment·credential query를 검사한다. 제품 SW나 cache/업데이트 정책·timeout/retry는 변경하지 않는다. 수정 후 오프라인 PC/mobile2개(5.9s)와 최종 전체72개가 통과했다.

pm-skills code-review의 correctness/security를 scope70/base4686ceb4에 순차 적용한다. 로비 제안→수락된 예약→취소/역순 조회, Auth private generation→WS 최초/후속 이벤트→계정 변경/철회, input UUID/epoch/ACK→공개 delta/기록과 UI/activity/locale/dispose를 함께 조사한다. 버퍼/프레임/동시 입력/기한·unknown DTO·고정 origin·명시적 재시도의 경계를 검토한다. 최종 diff 결과는 PR과 일별 review에 기록한다.

운영16×16/40 판과 미니PC·글로벌 RTT 예산 #26, 실제 OAuth 키/계정 #42, 사람 관찰 #27은 아직 검증하지 않았다. 친구 방 UI는 다음 #17 하위 작업이고 자동 재접속/명령 재전송·30초 유예의 클라이언트 출구는 #18이다. 상위17/18은 이 작업만으로 종료하지 않는다.
