# #10 Atomic UI·8언어 shell 검증

FR11/NFR01/03/06 · TS19/26/36. [ADR0014](../adr/0014-atomic-shell-locale-and-preferences.md), 코드 `e4af79772c693535cc03944c5a811bfe8b826547`. 선행 PR36/#9와 M1 종료를 확인한 뒤 시작했다.

design_layout의01/04/13/16을 참고해 격자 배경·4 심볼 로고·cream/amber·카드·큰 CTA·PC 장식 보드/모바일 단일 column을 구현했다. 실제 계정 닉네임/서버 정상/공식 daily 상태를 만들어 표시하지 않는다. 장식 판은 공개 고정 패턴이다. 나머지 디자인의 게임/인증/정책 상태는 후속 이슈로 연결한다.

Atomic Atoms/Button/Card/Icon/Dialog, Molecules/LanguageSelect/NavLink/ModeCard, Organisms/Header/AccountCard/DecorativeBoard, Template/AppShell, Pages/Home/Rules/Settings/Status를 분리했다. main이 I18nPort/NavigationPort/PreferencesPort/지연 PracticeCore factory를 주입한다. tokens.json→CSS/Pixi 색 TS를 생성·freshness 검사하며 공개 DEFAULT_RULES는 Rust snapshot에서 생성한다. UI에 게이지/타이머/기절 규칙을 다시 적지 않았다.

en common46개 키와7언어의 정확한 키·placeholder를 검사했다. i18next26.4.2는 영어+현재 언어만 로드하고 추가 locale를 지연 로드한다. URL 우선→루트에서 저장 선택→브라우저→en, 번체 Chinese→en fallback, route/query/hash 보존을 검증했다. locale chunk 실패 시 영어로 표시한다. 시간/수치는 Intl, 텍스트는 DOM escaping을 사용한다. 원어민/정책 검수 완료를 주장하지 않는다.

Red는 URL ko가 en으로 선택됨, 차단 저장소의 설정 업데이트 무시, 언어 control 부재, 실제 Chromium의 modal Tab 초점 이탈, 잘못된 JSON을 저장소 차단으로 오인하는 경우를 확인했다. Green으로 수정했고 저장소 읽기/쓰기 실패·2048자 초과/잘못된 JSON은 메모리/default fallback한다. 메모리 유지에는 탭 종료 시 설정이 사라짐을 안내하며 이후 쓰기 성공 시 정상 저장 상태로 복구한다.

- Rust73개·Vitest12개·실제 Chromium desktop/mobile20개 통과. core 소스는 #9 이후 변경하지 않았다. 로컬 ignored PostgreSQL1개는 필수 database CI가 실제 실행한다.
- 8locale URL, 360/390/768/1024/1440 폭의 긴 독일어 화면에 가로 넘침0. mobile은 Pixel7 에뮬레이션이며 실제 기기 검사와 구분한다.
- 언어 변경의 초대 code/hash 유지, reload 뒤 설정 유지, native modal Tab wrap·Esc·opener 초점 복귀, system/user reduced-motion, 고대비 적용, 광고/auth 요청0을 검사했다.
- base/high-contrast의 실제 text/muted/danger/success/number1 대 background/surface/raised와 accent/on-accent 모두4.5:1 이상을 계산했다. 주요 control44px 이상이며 키보드 focus ring·skip link·native label을 사용한다.

웹 TS/TSX 전체(미import source 포함, 테스트/setup만 제외) V8 line163/194=84.02%, statement177/212=83.49%, function69/77=89.61%, branch78/113=69.02%. CI에서 line80%를 강제한다. Dialog/main/Worker의 실제 브라우저 실행과 jsdom coverage는 구분하며 전체 앱 branch80%를 달성했다고 주장하지 않는다. [측정 데이터](web-shell-summary.json).

entry JS gzip25677byte, 추가 locale 각각 약1.3~1.5kB, lazy core665byte/worker591byte. assets의 모든 JS를 합해 gzip36903byte로 initial200KiB 예산보다 작은 보수적 상한이다. WASM와 public glue는 지연 로드하며 별도 측정한다. Python gzip level9·mtime0을 사용했다. LCP/FPS/실호스트 성능은 #26에서 실제 조건으로 측정한다.

검토용 실제 화면: [PC1440px](screenshots/10-home-chromium.png), [Pixel7 viewport](screenshots/10-home-mobile.png). 이미지 원본은 그대로 보존했다. 픽셀 일치·실기기 성능 검증으로 확대 해석하지 않는다.

scripts/check.ps1의 fmt/clippy·Rust/WASM·Rust DTO/wasm-bindgen/fixture/token freshness·Prettier/Oxlint/typecheck·web coverage/build·browser·문서/Mermaid34개가 모두 통과했다. 리뷰에서 DI 경계, public rules/장식 판, 언어 선택/저장소 fallback, 초점·실제 렌더·추가 청크를 확인했다. 필수6개 CI가 최신 PR head에서 통과한 뒤 위임에 따라 병합한다.
