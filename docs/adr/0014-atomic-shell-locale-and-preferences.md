# ADR0014: Atomic shell·locale·접근성 설정 경계

- 날짜: 2026-10-07
- 상태: 채택 — #10, FR11/NFR01/03/06, TS19/26/36. 선행 PR36/#9 병합·종료, M1 종료 확인.

## 결정

design_layout의 PC/모바일 홈·설정·언어를 공통 semantic token과 Atoms→Molecules→Organisms→Templates→Pages로 구현한다. 색 원천 JSON에서 CSS custom properties와 Pixi용 숫자 TS를 생성·freshness 검사한다. 로고/장식 보드는 공개 정적 패턴이며 실제 match 정보를 사용하지 않는다. main composition root가 I18nPort·NavigationPort·PreferencesPort·PracticeCore factory를 주입한다. 페이지에 storage/network/WASM 규칙을 직접 넣지 않는다.

en common JSON의 키·placeholder 계약을 나머지7언어와 검사한다. i18next adapter는 영어+현재 locale만 초기화하고 locale module을 지연 로드한다. 선택 URL은 `/en/`, `/ko/`, `/ja/`, `/zh-CN/`, `/es/`, `/pt-BR/`, `/de/`, `/fr/`다. URL이 저장/브라우저보다 우선, 루트만 자동 감지한다. 번체 Chinese는 영어 fallback한다. 언어 변경 시 route/query/hash와 기존 세션을 보존한다. 텍스트는 DOM에서 이스케이프하며 Intl을 사용한다.

언어·reduced motion·고대비·zoom은 서비스에 필요한 설정으로 bounded localStorage JSON에 저장한다. 읽기/쓰기 실패는 메모리 fallback하고 게임을 막지 않는다. 필수 저장소 고지는 설정에서 볼 수 있고 인증 CMP/선택 광고 동의와 혼동하지 않는다. CMP/정책은 #21에서 연결한다. system reduced-motion과 사용자 설정을 함께 적용한다. sheet/dialog는 native modal·Esc·초점 복귀, 주요 조작44px 이상, 모바일360/390·768/1024/1440 경계를 검사한다.

홈의 계정/연결 상태는 실제 값만 표시한다. 이 단계의 online unavailable 상태에서 로그인 성공·서버 정상·검증된 daily 결과를 만들지 않는다. 튜토리얼/로컬 게임/daily/auth의 실제 행동은 #12/#13/#15 이후 같은 route/DI에 연결한다. UI에 개발 세부사항을 표시하지 않는다. 초기 JS gzip≤200KiB는 전체 critical graph 기준으로 실제 계산하며 WASM·추가 locale는 별도로 보고한다.

[i18next API](https://www.i18next.com/overview/api), [타입 계약](https://www.i18next.com/overview/typescript), [native dialog](https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Elements/dialog)를 확인했다. 핵심 규칙/정책 번역의 원어민·법률 검수와 실기기 성능은 후속 검증이며 자동 테스트로 완료를 가장하지 않는다.
