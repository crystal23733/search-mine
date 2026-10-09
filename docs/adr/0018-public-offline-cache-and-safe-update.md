# ADR0018: 공개 정적 오프라인 캐시·다중 탭 안전 업데이트·제출 대기

2026-10-09 #66 회귀 조사: 기존 검사는 실제 배포의 고정 URL·새 bytes 대신 worker URL의 쿼리를 바꿨다. 두 idle ACK·동일 client 집합 뒤 활성화가 끝나지 않는 trace를 보존했다. 활성화 Promise를 message lifetime에서 분리한 후보도 추적 없는 모바일 반복에서 실패하여 폐기했다. 제품의 prepare/집합 재확인·게임 시작 lock·controllerchange 재로드·실패 release·15초 복구는 그대로 유지한다. 두 탭 검사는 자체 loopback 정적 fixture에서 실제 production 산출물을 제공하고, 같은 service-worker.js의 bytes만 변경하여 실제 배포를 재현한다. fixture는 각 시험의 임의 포트·메모리 revision·자동 close를 사용하고 공유 산출물을 수정하지 않는다. [공식 lifecycle](https://web.dev/articles/service-worker-lifecycle#avoid-changing-the-url-of-your-service-worker-script)과 [Playwright 제한](https://playwright.dev/docs/service-workers#known-limitations)에 따라 worker 업데이트를 URL 변경이나 route mock으로 대체하지 않는다. 반복/전체 검사가 통과하기 전에는 해결로 보고하지 않는다.

- 날짜: 2026-10-07

업데이트 검증의 브라우저 차이도 조사한다. 기본 chromium-headless-shell에서 고정 URL 시험도16개 중3개가 실패했고 임시 worker pending 계측에서 await skipWaiting 앞·이전 worker의 JS waitUntil pending0을 확인했다. Chromium의 native no-work/renderer-idle 조건과 JS pending0은 같은 관측이 아니므로 제품 결함의 원인으로 단정하지 않는다. [Playwright 공식 browsers](https://playwright.dev/docs/browsers#chromium-new-headless-mode)의 `channel: chromium`은 일반 Chromium의 새 headless 모드다. 오프라인/다중 탭 lifecycle 검사를 이 모드에서 대조하고 제품 SW는 변경하지 않는다. 라이브러리/브라우저 버전을 낮추거나 시간 제한/검사를 완화하지 않는다. shell 실패 사실과 새 모드의 실제 반복/전체/CI 결과를 기록한다.
- 상태: 채택 — #14, FR10, TS18/19/36. PR40/#13 병합·종료 확인.

## 결정

#66의 계측 제거/일반 Chromium 새 headless 검사에서 PC·mobile 각8회16개가1.7분에 통과했다. 제품 SW/adapter/coordinator diff0이며 전체/CI 결과를 추가 확인한다.

Service Worker는 build revision으로 관리하는 정적 shell/index·JS/CSS·8locale chunk·public Rust WASM/glue만 precache한다. 캐시한 Rust/RNG/rules는 UTC 공개 데일리를 재현한다. API 응답·OAuth/session/cookie·POST·WS·광고·외부 origin은 캐시하지 않는다. 캐시 없는 첫 오프라인 접근은 보장하지 않고 캐시/설치/저장소 실패도 온라인 로컬 플레이를 막지 않는다. 연결 신호는 서버 인증·가용성·매치 권위를 뜻하지 않는다. 온라인 매치를 오프라인 코어/봇으로 승격하지 않는다.

VitePWA injectManifest와 직접 작성한 SW를 사용한다. manifest:false·injectRegister:false이며 native registration port를 composition root에서 연결한다. Workbox precache revision key는 같은URL의 old/new core를 분리하고 새 install 실패는 기존 active 캐시를 유지한다. 런타임 API cache와 광고 SDK는 추가하지 않는다. 의존성은 버전 인자 없이 최신 안정으로 설치하고 실제 Vite/TS 호환성을 검사한다.

자동 skipWaiting이나 판 중 재로드를 사용하지 않는다. 로컬 controller는 loading부터 unmount까지 activity lease를 잡으며 결과에서 홈으로 이동해 업데이트한다. 완료 replay/record write는 별도 lease로 끝까지 보호한다. 사용자의 업데이트 요청에서 SW가 scope window 최대16개에 token+MessageChannel prepare를 보낸다. idle client는 새 core 시작을 막는 lock을 먼저 잡고 ACK, busy/누락/2초 timeout은 실패로 닫는다. 준비 뒤 client ID 집합을 재확인하고 모두 idle인 경우에만 skipWaiting한다. 실패는 lock release, 응답 누락에는15초 lease 복구를 둔다. old controller를 가진 잠긴 client만 controllerchange 뒤 재로드하고 첫 설치 claim은 재로드하지 않는다. 새 판 시작 guard가 lock을 확인하므로 ACK 직후의 시작 경합도 차단한다.

IndexedDB v2의 records+pending은 같은 transaction에서 첫 local clear와 replay candidate를 저장한다. 기존 v1 개인 기록은 bounded migration으로 pending에 연결한다.30개 상한·strict v1/unverified 형식·denied/quota/blocked/timeout memory fallback을 유지한다. pending은 계정 없는 제출 후보이고 공식 완료나 순위가 아니다. 새 공식 인증/검증 API는 #15/#19이며 그 전에는 후보를 보존하고 전송 성공을 만들지 않는다.

제출 service는 명시적으로 확인한 현재 계정과 연결 회복을 port로 검사한 뒤 attempt UUID를 재사용한다. 계정/연결 변경·실패는 후보 유지, 서버의 만료/unsupported는 pending만 제거하고 개인 기록을 유지한다. 같은 service의 중복 flush는 합치고 재시도/다중 탭의 최종 중복 효과는 서버 idempotency로 막는다. 클라이언트가 verified/공식시간을 결정하지 않는다.

설정에서 개인 기록+pending을 함께 삭제하고 공개 cache를 삭제/재활성할 수 있다. cache 삭제도 모든 탭 idle gate를 적용하고 자체 prefix만 지우며 unregister한다. 필수 local cache-disabled preference와 실패 시 메모리 대체를 사용한다. 삭제 실패·미캐시 상태를 안내하고 게임/언어/광고 동의를 강제하지 않는다.

TDD는 activity lease/시작 경합·busy/누락/새 client 업데이트·DB upgrade/원자 큐/손상·명시적 계정/재시도/만료 정책을 검사한다. 실제 production preview에서 cache 상태/8locale/WASM·offline daily/reload·대기 저장·최초 미캐시 실패·two-tab busy 업데이트 차단·개인정보/API/광고 cache0과 삭제를 검증한다. 개발 Vite 서버는 SW를 켜지 않으며 별도 production preview를 사용한다. M2의 실제 사람 E1/E2는 #27 수동 게이트로 유지한다.

## 기술 근거

실제 latest 설치는 VitePWA2.0.0, Workbox7.4.1, assets-generator2.0.0이다. PWA 선언의 optional assets-generator 의존성을 명시하고 DOM 앱·SW·Vite 설정 tsconfig를 분리했다. upstream unconfig7.5.0의 미정의 Args 선언은 runtime load(force=false)에 맞게 boolean으로, sharp-ico0.1.5의 미사용 sharp-bmp type import는 제거하는 pnpm 타입 패치만 적용했다. runtime 코드는 바꾸지 않고 버전을 내리지 않았다. patches와 lockfile을 커밋하고 frozen 설치로 재현한다. skipLibCheck로 오류를 숨기지 않는다.

cache state가 render와 effect subscribe 사이에 먼저 갱신되는 경합을 실제 offline reload에서 확인했다. useSnapshot은 subscribe 직후 최신 read도 반영한다. offline startup에서는 새로운 네트워크 등록을 요구하지 않고 기존 active registration을 조회한다. SW 캐시는 브라우저가 삭제할 수 있으며 installed 상태가 영구 보존을 보장하지 않는다.

[Workbox precaching](https://developer.chrome.com/docs/workbox/modules/workbox-precaching), [VitePWA injectManifest](https://vite-pwa-org.netlify.app/guide/inject-manifest.html), [prompt update](https://vite-pwa-org.netlify.app/guide/prompt-for-update.html), [SW skipWaiting 표준](https://w3c.github.io/ServiceWorker/#service-worker-global-scope-skipwaiting)을 확인했다. 표준 skipWaiting 자체는 모든 게임이 idle임을 보장하지 않으므로 별도 prepare/lock 경계를 구현한다.
