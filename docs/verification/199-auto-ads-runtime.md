# #199 · 자동 광고 script와 문서 경계

[ADR0091](../adr/0091-adsense-auto-ads.md) · FR12/13·NFR01/02/05/07/08·TS18/20/23/24/25/30.

선행196/PR197 MERGED/CLOSED·developd78e6b9 clean 확인. 사용자의 자동 광고/교체 가능한 script 지시를 구현한다. 제품 코드보다 먼저 시나리오/구현 경계를 기록했다.

문서가 시작된 URL이 보호 경로/query/hash이거나 같은 문서에서 보호 화면을 방문했다면, 나중에 홈으로 돌아와도 SDK는 새 공개 문서에서만 활성화한다. 이로써 이전 게임 DOM/URL 정보가 남은 문서에서 load를 시작하지 않는다. 메모리 전용 학습/설정 환경은 광고를 생략해 자동 문서 이동이 학습 선택을 잃게 하지 않는다. 초기인증 loading 중에는 광고를 시작하지 않는다. activity 업데이트 locked·hidden은 신규 load를 막지만 이미 시작한 문서를 그 이유만으로 reload해 SW 조정을 방해하지 않는다. 현재 권한/owner/온라인/게임busy와 BFCache는 문서 종료 경계를 적용한다.

TDD·실브라우저/생성물/회귀·coverage·실제 코드 리뷰 결과는 실행 후 기록한다. 실제 CMP/ID/사이트승인/Google 설정/CSP/ads.txt/실 SDK는147/상위21/22의 미완료 검수이며83/198의 기존 간헐 실패와 구분한다.

접근성 경계: 공개 문서의 고정 skip 링크 `#main`은 같은 공개 내용을 가리키므로 허용한다. 그 밖의 hash와 query는 제외하며 hashchange도 현재 문서 gate를 검사한다. 이를 통해 키보드 skip이 불필요한 새로고침을 만들지 않으면서 알 수 없는 fragment의 광고 실행을 차단한다. ADR0091의 query/hash 제외는 이 고정 접근성 앵커 예외를 따른다.

## 실제 구현·검증

AutoAdsScriptPort와 실제 AdSense async/crossOrigin script adapter를 root에 연결했다. VITE_ADSENSE_CLIENT_ID가 없거나 형식이 다르면 외부 script0이고 실제 CMP factory가 없는 현재 앱은 계속 광고0이다. loader는 단1회·3초 기한·AbortSignal/오류/listener 정리, 임의 URL/inline script·수동 광고 단위/adBreak를 만들지 않는다.

- 실제 행동Red3(허용script1회·로더생성·게임publish전문서이동 실패) 뒤 Green3. 현재목적/초기auth/활동/저장/8locale·전보호경로/개인/queryhash·실패/취소/중복/late완료·navigation/popstate/dispose를 확장했다.
- 코드리뷰로 BFCache의 이전retiring 상태가 남는 경계를 발견해 Red1 뒤 보완했다. SDK가 시작된 복원 문서는 새문서를 열고, SDK 없는 복원 문서는 기존SPA로 복귀한다. 합성 persisted 이벤트는 단위 계약 검증이며 실제 BFCache 발생을 실측했다고 주장하지 않는다.
- 고정 접근성 #main의 불필요한reload와 알수없는hash 변경의 누락을 Red2로 확인한 뒤 정확한 앵커 예외/hashchange 재검사를 적용했다. 실제 브라우저에서 skip링크의 #main focus·같은SDK문서 유지도 확인했다.
- 전체 웹423PASS, 라인91.37%(3836/4198). 실제 PC/mobile 자동광고 계약10개가3.2초에 통과했다. 이전8개3.3초 뒤 복원/fragment 보완과 새build로 다시 검증했다. 외부 Google 요청은 테스트 응답만 사용했다.
- Rust 전체464PASS/실PG185ignored, fmt/clippy(-D warnings)/WASM target/생성TS·선언·기존nativefixture check·lint/typecheck/format/build·문서243/Mermaid34 통과. 로컬 실PG/LLVM/전체PowerShell/전체브라우저는 미실행이며 CI와 구분한다.

194의 미사용 Rust/protocol/WASM 정책 API·광고 전용 fixture/helper/parity test를 제거했다. 기존 native 게임fixture·규칙/서버/DB/선택event의과거불변계약·SW/브라우저 flags/retry/timeout·의존성은 바꾸지 않았다. 당시 ADR/검증/기획 원문을 보존하고 새 기본 설계가 이를 대체함을 기록했다.

## 실제 코드 리뷰와 운영 한계

script를 부르기 전에 document를 실행 가능 상태로 표시하고 실패/취소에도 되돌리지 않는다. 보호 화면은 pushState/구독 통지 전에 same-origin 새 문서로 이동하며 이동 실패 시 이전 안전 화면을 유지한다. 보호 문서에서 공개 화면으로 돌아와 처음 SDK를 켤 때도 새 공개 문서를 먼저 연다. 메모리 전용 학습/설정에서는 광고를 생략하고 기존SPA를 유지한다. 계정/동의설정은 원래 Header/AccountCard의 보호route로 들어가므로 SDK 문서에서 권리변경/CMP재열기를 시작하지 않는다.

현재 권한/owner/온라인/게임활동 변경·복원은 abort와 문서 종료 경계를 적용하고 노드 삭제를 SDK unload로 간주하지 않는다. 기본미설정 상태의 navigation은 기존 동작이며 새guard는throw/false에publish하지 않는다. 광고 loader의 실패를 게임/계정 오류로 전파하지 않고 script load를 impression으로 기록하지 않는다. 테스트용grant/실행마커/게시자ID는 test 전용 HTML/module에만 있고 production entry는 이를 import하지 않는다.

[운영 설정](../workflow/auto-ads-configuration.md)의 실제 사이트승인/ID/CMP·인페이지Auto ads/overlay제외·page exclusions/CSP/ads.txt·실제Google/TCF/PCmobile 검수는147/상위21/22에서 수행한다. 이 구현은 실제 광고승인/노출/수익·83/198의 기존 간헐 실패 해결을 증명하지 않는다. CI/정확head·최종리뷰/병합은 PR과 당일로컬review에 기록한다.

로컬 새모듈 라인은 script34/34=100%, browser86/89=96.62%다. 초기계정null/type-only Window필드·HTML형식 오류는 수정했으며 컴파일/형식 실패를 행동Red로 세지 않았다.
