# ADR0015: 공개 보드의 Pixi 표시·DOM 조작 경계

- 날짜: 2026-10-07
- 상태: 채택 — #11, FR03/04/05, NFR03/06, TS04/09/26. PR37/#10 병합·종료 확인.

## 결정

BoardOrganism은 GameView의 own.cells·공개 규칙 dimensions·phase/stun만 받는다. CellAtom DOM grid와 Pixi CellDisplay는 동일 PublicCell을 표시하며 history/정답/lie 여부를 추론하거나 공개하지 않는다. 선택·mode·viewport는 UI 상태, 게임 명령은 PublicAction으로 외부 controller에 전달한다. 공격은 별도 UI로 연결하고 보드에서 상대 target 선택을 제공하지 않는다. #12에서 실제 WASM 로컬 대전에 이 컴포넌트를 연결한다.

composition root가 비동기 BoardRendererFactory를 주입한다. Pixi adapter는 고정 pool의 Graphics/Text를 재사용하며 public cell 또는 테마 변경만 갱신한다. 자동 ticker를 끄고 변경 시 수동 렌더한다. 기기 pixel ratio는2로 제한하고 실패하면 조작 가능한 DOM 표시로 전환한다. 초기화 중 unmount/실패와 destroy를 처리하며 홈에서는 Pixi를 로드하지 않는다. [Pixi Application 공식 문서](https://pixijs.com/8.x/guides/components/application)와 [수동 렌더](https://pixijs.com/8.x/guides/components/renderers)를 확인했다.

DOM은 role grid/row/gridcell, 좌표·공개 상태/숫자/깃발 label, roving tabindex를 제공한다. Arrow/Home/End로 이동, Enter/Space 실행, A/F/O로 지목/깃발/열기 mode, Shift+F10/contextmenu로 셀 메뉴를 연다. 공개 playing이 아니거나 stun_ms>0이면 명령만 막고 초점·보기·동기화는 계속한다. UI에서 승패나 안전/지목 성공을 확정하지 않는다.

격자는 [WAI table grid 패턴](https://www.w3.org/WAI/ARIA/apg/patterns/grid/examples/data-grids/)을 따른다. 정적 lint가 table의 interactive grid를 거절하는 규칙과 명시적 td/gridcell을 중복으로 판정하는 규칙만 해당 컴포넌트에서 근거를 적어 예외 처리한다. 명시적 gridcell은 td를 일반 cell로 노출하는 DOM 테스트 소비자와의 일관성을 유지한다. Chromium 접근성 역할과 키보드 동작을 실제 검사하고 스크린리더 수동 검수는 #23에 남긴다.

cell pitch48px를 기본으로1~2배 확대하고 scroll viewport/minimap으로 이동한다. 16열을 좁은 화면에 압축하지 않는다. Pointer gesture port는 clock/timer와 callbacks를 주입하며 450ms long press→메뉴, 이동 threshold8px→pan, 다중 pointer→pinch를 구분한다. long press/pan/pinch/cancel 뒤 click은 명령을 발생시키지 않는다. pinch와 zoom은 범위를 제한하고 center를 보존한다. 키보드 선택은 화면 안으로 scroll한다. reduced-motion에서는 이동 animation을 사용하지 않는다.

실제 Chromium desktop/mobile에서 public fixture를 test 환경에서 주입해 canvas·DOM·기절·메뉴·gesture·초점과 resize를 검사한다. pool/dirty 업데이트와 timer 취소는 DI 테스트로 검증한다. 개발 PC 브라우저의 render/animation frame 시간을 표본·환경과 기록하며 실제 모바일60fps/미니 PC 성능 달성으로 보고하지 않는다. game bundle은 지연 청크와 초기 경로를 별도로 측정한다.
