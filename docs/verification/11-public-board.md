# #11 공개 Pixi 보드·터치·키보드 검증

FR03/04/05, NFR03/06 · TS04/09/26 · [ADR0015](../adr/0015-public-board-rendering-and-input.md). 코드 `a1e50cbc6c807cb33de6ca34e600fcfbb3e71331`, 선행 PR37/#10 병합·종료 뒤 구현했다.

BoardOrganism과 CellAtom은 GameView의 공개 셀·규칙 dimension·phase/stun만 표시한다. 원래 공개 숫자를 그대로 읽고 secret Board/seed/overlay/true number는 받지 않는다. 선택·mode·viewport를 게임 규칙과 분리해 PublicAction을 외부 controller로 전달한다. 실제 로컬 대전/튜토리얼 페이지 연결은 #12다. 상대 보드/공격 target 선택은 제공하지 않는다.

Pixi8.22.0을 버전 인자 없는 pnpm add로 설치했다. renderer factory를 composition root에서 지연 주입하며 pool Graphics/Text 재사용·dirty cell 갱신·수동 render·DPR≤2를 구현했다. 같은 표시에서 render0회, 단일 셀 변경에서 해당 display만 갱신, 테마 변경에서 pool 유지와 전체 재색칠을 검사했다. 초기화/render 실패 시 자원 해제 후 DOM으로 계속 조작하며 unmount 중 초기화 완료도 dispose한다. 고대비 숫자 토큰은 JSON에서 생성한다.

native table grid에 좌표·공개 숫자/깃발/상태, roving tabindex, Arrow/Home/End/Control+Home/End, O/F/A mode, Enter/Space, Shift+F10 메뉴를 제공한다. phase/stun은 명령만 잠그며 초점과 공개 갱신은 유지한다. 재현 가능한 타이머 포트에서 tap450ms 구분·8px pan 취소·pinch·pointer cancel/dispose를 확인했고 실제 CDP touch에서 long press/pan/pinch 뒤 trailing open0을 검사했다. 메뉴 Esc·초점 복귀, minimap/zoom과 resize도 검증했다.

초기 Red는 명령/menu/pan/pinch 부재, 공개 DOM/roving/기절 부재, 캔버스/pool 부재였다. 리뷰에서 render 실패의 즉시 dispose 누락을 Red로 확인해 수정했다. 실제 Chromium은 Pixi destroy 뒤 app.canvas getter 접근 실패를 발견했고 캔버스 참조를 보관해 수정했다. 화면 리뷰에서 Preact11의 숫자 style에 px가 자동 추가되지 않아 canvas1066px/DOM768px가 어긋난 것을 발견했다. geometry Red를 추가해 width/height를 명시적 px로 수정했고 desktop/mobile에서 canvas768×768, 셀44×44 일치를 검증했다. 테스트에서 Preact 중복 import는 별도 test 진입점으로 해결했다.

- 최종 scripts/check.ps1 종료0: fmt/clippy·Rust73개·WASM·DTO/fixture/token freshness·format/lint/typecheck·Vitest27개·build·실제 desktop/mobile Chromium28개·문서/Mermaid34개 통과. PostgreSQL1개는 로컬 ignored이며 필수 database CI에서 실제 확인한다.
- 웹 소스 V8 line438/484=90.49%, statement469/523=89.67%, function129/142=90.84%, branch256/323=79.25%. 미import main을 포함하고 테스트/setup만 제외했다. 전체 앱 branch80%를 주장하지 않는다.
- 360/390/768/1024/1440의 보드 resize와 가로 넘침0, keyboard 마지막 셀 scroll·zoom을 확인했다. mobile은 Pixel7 viewport/DPR2.625 에뮬레이션이다. [실제 PC 보드](screenshots/11-board-chromium.png)·[모바일 보드](screenshots/11-board-mobile.png)는 컴포넌트 테스트 화면이며 대전 HUD까지 구현한 화면은 아니다.
- 캡처 실행 중 모바일1개가 page.goto 단계에서 Windows ERR_NO_BUFFER_SPACE로 실행되지 않았다. 영향받은 touch 검사를 별도 실행해 통과했고 최종 전체28개 검사도 재실행해 통과했다. 제품 실패를 재시도로 숨기지 않았다.

Vite manifest의 **정적 imports closure**를 따라 초기 JS gzip26496byte, 현재 언어 추가 최대1919byte, 전체 초기 상한28415byte를 계산했다. 모든 renderer backend·locale·core/worker assets JS 합203220byte는 지연 경로까지 포함한 보수적 상한이다. 현재 game factory 의존성 측정이며 #12 실제 game UI는 추가 측정한다. Python gzip level9/mtime0, [원자료와 asset 목록](public-board-summary.json). public WASM/glue는 #9의 별도 자산이다.

Windows11/Ryzen9 3900X/headless Chromium, Playwright2 workers에서 각120개의 단일 dirty-cell CPU render submission과 RAF 간격을 기록했다. CPU submission p95 PC2.3ms/mobile2.0ms, RAF frame p95 PC50.1ms/mobile100ms였다. GPU 완료 시간이 아니며 두 rendering context와 software headless 환경의 표본이다. **이 환경은16.7ms/60fps 목표를 충족하지 않았고 실기기60fps 달성을 주장하지 않는다.** 실제 저사양 모바일·미니 PC·LCP·GPU/메모리는 #26, 스크린리더 수동 검수는 #23에서 측정한다. raw120개·UA·DPR를 원자료에 보존했다.

리뷰에서 public 정보 경계, DOM/Canvas geometry, 입력 중복, async lifecycle/fallback, 최신 Pixi API·번역 키65개·token freshness를 확인했다. native table/grid와 명시적 gridcell의 정적 lint 충돌은 ADR의 WAI 근거에 따라 해당 컴포넌트 두 규칙만 예외 처리했다. 최신 PR head의 필수6checks 통과 후 위임에 따라 병합한다.
