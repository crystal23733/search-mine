# #84 단위 테스트 준비·DOM 조회 진단

NFR05/08 · TS18/26/30 · [테스트 설계](../design/14-testing-strategy.md). 선행 #85/PR86은 develop a7c3099에 병합·종료했다. #83 제품 업데이트 원인은 미확정 OPEN이며 이 작업과 독립적이다.

## 기준과 계획

#80 로컬 전체176개는5fail/171pass36.11초, 같은 설치의 선행 develop a223bd4f 직접 실행도2fail/160pass24.96초였다. 친구/큐는 Preparing board에서 기본 findBy1000ms, 로컬 시나리오는 전체5000ms 제한에 실패했다. #85 최종 로컬 전체176개는 queue5000ms 1fail/175pass17.66초였지만 독립 CI176개는21.02초에 통과했다. 환경 부하의 정확한 원인은 확정하지 않는다.

페이지 import 준비와 실제 렌더/권한·256셀 accessible role/name 조회 시간을 먼저 측정한다. 테스트 동작·모든 기존 단언을 보존하면서 준비 hook 또는 독립 행동 시나리오 분리를 적용한다. 제품 lazy loading·규칙·DOM과 timeout/retry/worker 설정은 변경하지 않는다. cold route/실제 WASM·WS는 기존 production Chromium E2E로 확인한다. 로컬 전체/coverage·CI·PR·병합 결과는 실행 뒤 추가한다.

임시 observer의 전체176개는3fail/173pass27.30초: 큐·practice 초기findBy가1000ms, daily 전체가6610ms로 실패했다. daily 측정6598ms 중 B1 accessible-name 조회3회1350/1117/852ms였다. 소규모8개는 통과해 동시 실행 범위의 차이도 확인했다. 모듈 hook과 큐/데일리 독립 시나리오12개는16.02초 통과했지만 전체gate62137은4fail/176pass180개40.61초였다. Rust/native/WASM·types/fixture/tokens·format/lint/typecheck까지 통과 뒤 실패해 전체gate 성공은 아니다.

전체 실행에서 jsdom32회 생성338.90초가 추적 시간의62%였다. 준비 hook만으로는 충분하지 않았다. 또한 친구방은 정상500ms status poll을 전체 lobby 호출3회에 섞어4회로 실패했다. DOM이 필요 없는 fake-port 테스트의 Node 환경 명시, DOM 전용 setup, poll과 사용자 명령 관측 구분을 다음 수정으로 정했다. OS/사용자 프로세스 부하의 원인은 미확정이며 변경하지 않는다.

## 변경과 검증

- fake Worker/EventTarget·Clock/Storage/HTTP/Socket 포트만 쓰는21개 파일에 Node 환경을 명시했다. 나머지11개 DOM/브라우저 테스트는 jsdom과 기존 modal API·cleanup을 유지한다. 파일 격리·worker 수·timeout·retry·라이브러리 버전은 그대로다. Node 실행에서 DOM 의존성을 mock으로 숨기지 않았다.
- 큐는 배정/해제·권한 복구·복구 후 상대 유예/locale/서버 저장 결과로 나눴다. 모든256셀 role 검증과 readonly Attack·두 번째 connect·서버 결과 전 recording 없음·notice가 보드 앞임·결과 뒤 activity hold·home 해제를 유지했다. 세 번째 시나리오에도 실제 suspend/resume을 남겨 복구한 연결에서 locale와 결과를 처리하는 원래 순서를 검증한다.
- 데일리는 locale/자정/공유/worker 교체·서로 다른 날짜의 두 기록·같은 날짜 첫 기록 보존으로 분리했다. 마지막은 개수뿐 아니라 원본 기록 전체가 그대로임을 비교한다. 연습은 거절/결과/완료 worker 교체와 난이도/초대 URL/해제로 나눴다. 셀을 CSS나 hidden 조회로 바꾸지 않았다.
- 친구방의 초기 status1회는 유지하고 정상 poll과 room_join/ready를 구분해 정확한 사용자 명령을 비교한다. ready 때 AbortSignal이 locale 변경 후에도 철회되지 않음을 확인해 controller 재생성도 탐지한다. 입력/공유의 공개 locale·code와 정확한 취소 identity 검증은 유지한다.
- 21개 Node 분리 뒤 전체180개는179pass/연습5000ms1fail29.41초였다. 연습 분리 뒤181개는180pass/1fail24.67초: 새 정리 단언이 unmount 직후 effect 해제보다 빨랐다. 설치된 renderer의 unmount는 act를 감싸지 않아 연습 정리를 명시적 act로 완료했다. 제품 수명 코드는 변경하지 않았다.

최종 전체181개/32파일이23.48초에 통과했다. 웹 line2203/2517=87.52%, statements2349/2747=85.51%로 기존80% gate를 유지했다. 이 관측 한 번을 영구적인 무실패나 OS 지연 원인 해결로 일반화하지 않는다. 정리 콜백의 void 반환 타입 보정 뒤 최종 format/lint/typecheck·fresh build/PG17.4 실제 HTTPS 포함 전체85browser(2.4분)/재시도0·docs135/Mermaid34 실패0을 확인했다. Rust/native/WASM/type/fixture 검사는 앞의 전체gate에서 통과한 범위와 구분한다.

PR87 head9f4750d의 Product37948238867/Repository37949351458 최신6checks SUCCESS. 실제CI181unit15.37초/line87.52%·85browser4.9분/재시도0·PG18.6 DB26/7.52초·Auth99.48/Match99.31/Lobby98.37/whole92.69/core95 gate를 확인했다. PR87은2026-10-09T15:08:45Z squash merge/develop2db4ac7, #84 CLOSED15:08:46Z를 조회했다. #83 원인은 미확정 OPEN이다.
