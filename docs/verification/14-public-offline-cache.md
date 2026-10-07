# #14 공개 오프라인 캐시·안전 업데이트·제출 대기 검증

2026-10-08 · FR10, TS18/19/36 · [ADR0018](../adr/0018-public-offline-cache-and-safe-update.md) · [측정 JSON](offline-summary.json).

## 구현 결과와 신뢰 경계

production build만 SW를 등록한다. 공개 shell·모든 8언어·정적 JS/CSS·Rust WASM/glue를 build revision으로 precache한다. API/OAuth/session/POST/WS/광고와 외부 origin을 캐시하지 않는다. Vite 개발 서버에서는 SW를 사용하지 않는다. 네트워크 회복 신호는 인증이나 서버 가용성을 증명하지 않고 온라인 매치를 로컬 봇으로 전환하지 않는다.

다중 탭 prepare는 idle 탭의 새 게임 시작을 먼저 잠근다. busy·무응답·client 집합 변경·시간 경계에서 업데이트를 거절하고 lock을 푼다. loading부터 페이지 unmount까지 게임 lease, replay/DB 저장 완료까지 별도 lease를 유지한다. idle 승인된 탭만 controllerchange에 재로드하며 최초 claim은 재로드하지 않는다. 사용자 업데이트 버튼으로 시작하며 자동 판 중 업데이트가 없다. cache 삭제는 모든 탭 idle gate·자체 namespace·unregister를 적용하고 다른 앱 cache는 유지한다. 설치 중 삭제는 거절하여 설치 write와의 경합을 줄인다.

IndexedDB v2 records+pending은 첫 로컬 완료와 익명 replay를 원자적으로 저장한다. v1 기록의 bounded migration, 30개 상한, 손상·denied·quota abort·stalled open의 경고와 memory fallback을 유지한다. 정확한 attempt 응답만 pending을 제거하며 개인 기록은 남고 이후 연습은 재제출 후보를 만들지 않는다. 설정의 확인 버튼은 개인 기록과 대기 목록을 함께 삭제한다.

제출 policy는 명시적 현재 계정·session revision·연결을 검사하고 동일 attempt ID로 재시도한다. 중복 flush를 합치고 10초 timeout/AbortSignal을 둔다. 계정 변경·실패는 후보 유지, accepted/expired/unsupported는 pending만 제거한다. 현재 인증/공식 API를 연결하지 않았다. 실제 연결은 #15/#19이고 개인 데이터의 verified/time 순위 주장도 만들지 않는다.

## 실행한 검사

- `scripts/check.ps1`: Rust fmt/clippy/workspace tests/WASM·Rust→TS/fixture/tokens freshness·format/lint/typecheck·Vitest64·build·전체 Chromium50·문서91·Mermaid34 통과. DB 없는 local 기본 cargo의 ignored PostgreSQL 테스트는 통과로 세지 않았다.
- 리뷰의 최종 SW/설정 경계 보강 뒤 관련 format/lint/typecheck/build·Vitest64·production browser8을 다시 통과했다. latest head GitHub 필수6checks와 실 PostgreSQL/코어 coverage는 PR에서 확인한다.
- web V8 lines1075/1216=88.40%, statements1163/1334=87.18%, functions334/390=85.64%, branches667/845=78.93%. 전체 src와 미import main 포함, exclusion/threshold를 바꾸지 않았다. worker는 별도 타입 검사와 실제 production E2E로 검사하며 이 web coverage 수치에 포함되지 않는다.
- `pnpm install --frozen-lockfile`로 최신 의존성/타입 패치의 재현을 확인했다. VitePWA2.0.0·Workbox7.4.1·assets-generator2.0.0을 버전 인자 없이 설치했다. upstream의 선언 오류 두 곳만 pnpm patch로 고쳤고 runtime 변경·구버전 pin·skipLibCheck가 없다. unified patch 문법의 빈 문맥 공백은 `.gitattributes`에서 별도로 처리한다.
- 최종 문서 검사92개·Mermaid34 통과. 실제 gzip 측정: 초기 JS+최대 선택 locale38177bytes, 최종 SW6943bytes, precache41개/1268854bytes. precache 총량은 초기 critical JS 예산과 별개다.

실제 production preview4173 PC/Pixel7 Chromium의 네 가지 시나리오를 각2회 구성했다(8개):

1. precache의 public WASM 확인·8언어 offline navigation/reload·실제 Rust 데일리와 봇 시작.
2. countdown 뒤 native solver fixture의 실제64 입력으로 offline clear→개인 기록/pending의 reload→설정에서 확인 후 양쪽 삭제. 계정/email/password 없이 unverified replay가 저장됨을 확인.
3. 실제 두 탭: 다른 탭의 플레이 중 업데이트 거절·판 유지·홈 이동 후 양쪽 reload·자체 cache 삭제/unregister·다른 cache 보존·재활성·합성 credentialed API probe의 cache entry0.
4. 최초 미캐시 offline navigation 실패·SW 등록 거절에도 로컬 게임 작동.

초기 SW 없음에서 controller=false Red를 실행했다. store/queue·activity/coordinator·제출 policy·새 게임 guard도 compiled Red→Green을 확인했다. 빠른 offline active-registration 조회가 render/effect 구독 사이에 끝나 stale 안내가 생긴 실제 실패를 useSnapshot의 subscribe+즉시read로 해결했다. 처음 입력은 countdown에서 금지된다는 규칙에 맞게 playing을 기다리도록 E2E를 고쳤다. 재로드 관찰은 실제 load 이벤트를 기다리고 assertion 시간을 늘리지 않았다.

## 실제 화면과 제한

[PC 오프라인 데일리](screenshots/14-offline-chromium.png) · [모바일 데일리](screenshots/14-offline-mobile.png) · [PC 대기 기록 설정](screenshots/14-settings-chromium.png) · [모바일 설정](screenshots/14-settings-mobile.png). 제공된 디자인의 dark/accent 공통 tokens·반응형 카드·접근성 컨트롤을 유지했다. 모바일 설정과 PC 데일리를 직접 시각 검토했다.

새 worker URL을 실제 등록하여 activation gate를 검증했다. 이전 배포의 다른 WASM binary를 교체하는 전체 회귀 실험은 아니다. 캐시는 브라우저가 삭제할 수 있고 최초 미캐시 offline 방문은 지원하지 않는다. static precache 전체 크기와 초기 critical JS 예산을 구분한다. 실제 기기 FPS/미니 PC 측정은 #26, 실제 사람 E1/E2는 #27이며 M2를 그 전까지 완결했다고 주장하지 않는다.
