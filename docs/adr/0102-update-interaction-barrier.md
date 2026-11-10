# ADR0102: 업데이트 준비 ACK 전 입력 장벽

- 상태: 채택 — 승인된 #83·FR10의 입력 경합 수정, 전체 CI/병합 결과는 PR에서 확인
- FR10 · NFR01/02/05/06/07/08 · TS18/20/26/30
- 선행 #221/PR222 develop20dcda6 병합·종료, [재현 기록](../verification/83-activation-recovery.md)

## 근거

동일 pointer 최소 재현의 baseline은 PC/mobile 대조2/2 reload, 입력2/2 Warm Up·old재시작·load10초 실패였다. HTML inert 후보와 한 입력을 먼저 보낸 primed-inert 후보는 각각 입력2/2 reload·Warm Up0/old재시작0이었다. 기존 activity 잠금은 새로운 core 시작을 막지만 링크/설정 입력 자체를 막지 않아, 두 idle ACK 이후에도 브라우저의 링크 워밍업이 유발될 수 있다. 이 결과는 입력 경로의 대조이며 PR222의 불완전 native trace에서 원인을 새로 확정한 것이 아니다.

## 결정

기존 activity.prepare의 잠금 변화에 동기 구독하는 브라우저 입력 장벽 port를 composition root에 연결한다. 준비 성공 후 native dialog.showModal로 접근 가능한 모달을 먼저 열어 나머지 문서를 inert로 만들고, 실제 장벽이 열린 경우에만 prepare ACK true를 보낸다. 모달 API 실패/부재는 NACK와 원래 lock release로 끝내며 skipWaiting하지 않는다. 기존 token/client 재확인·busy 거절·2초 prepare/5초 guard·8초 request·15초 lease는 바꾸지 않는다.

장벽은 일반 페이지/게임 실행을 항상 막는 것이 아니라 모든 idle 탭의 짧은 준비 잠금에만 적용한다.8언어 상태 문구·디자인 토큰을 사용하고 ESC는 critical lock을 임의 해제하지 않는다. 성공 controllerchange는 기존 locked/controlled 조건으로 reload한다. 실패 release/기존 lease 만료/소유 adapter dispose는 모달·리스너를 정리하고 아직 유효한 원래 focus를 복원한다. 다른 dialog/DOM/저장소/등록/캐시/API·게임 규칙을 삭제하거나 변경하지 않는다.

ActivityPort와 UI 구현을 분리하고 OfflinePort는 장벽의 ready 상태만 의존한다. HTML 생성은 UI browser adapter가 담당하며 도메인은 DOM을 모른다. 표준 modal의 inert 동작으로 링크 탐색/워밍업 원인이 되는 새 입력을 잠그며 브라우저 내부 활성화 완료를 강제하거나 성공으로 추정하지 않는다.

## 검증과 완료 기준

기존 무장벽 동작에서 prepare ACK가 열린 UI를 허용하는 Red와, UI 장벽의 busy/lock/NACK·동기 ACK 순서·중복/lease/failure/dispose/focus/8언어를 먼저 확인한다. 최소 재현 native modal/primed-modal과 실제 production 두탭의 잠금 중 링크 좌표 입력·원래10초 reload를 대조한다. 기존 complete native 관측과 불완전 trace를 구분한다.

제품에 채택하려면 PC/mobile의 원래 두탭 시나리오를 retry0로 반복하고 게임 중 거절/첫등록/캐시삭제·8언어·기존 전체CI를 통과해야 한다. 재시도 성공이나 단순 Warm Up0을 원인 해결로 확대하지 않는다. 후보가 실패하면 원문 보존 후 #83을 열린 상태로 유지한다. 버전 pin/flags/기존 timeouts·retry 완화/수동 worker stop·start/캐시삭제/등록해제·강제탐색은 수정 경로에 넣지 않는다.

[Chromium exact revision의 activation](https://raw.githubusercontent.com/chromium/chromium/971a7443b0c9b0a9b2860529b33331b76077ec62/content/browser/service_worker/service_worker_registration.cc), [native start/idle](https://raw.githubusercontent.com/chromium/chromium/971a7443b0c9b0a9b2860529b33331b76077ec62/content/browser/service_worker/service_worker_version.cc), [HTML modal/inert](https://html.spec.whatwg.org/multipage/interactive-elements.html#dom-dialog-showmodal)을2026-11-10 확인한다.

## 실제 대조

원래 일반 production 시나리오16회는 변경 전에도 통과했다. 별도로 같은 실제 앱에서 링크 입력을 겹친8회는4통과/4load10초 실패였고, 실패 trace4개 모두 같은 임의 origin의 완전 native trace와 짝지어 Warm Up1/Restart1을 확인했다. 이는 기존 자연 실패와 같은 native 경합을 앱 입력으로 재현한 근거이며 과거 불완전 trace의 내부 원인을 소급 확정하지 않는다.

수정 후 같은 입력/기존 업데이트/8언어 장벽 검증24회가 통과했고, 추가 입력/기존 시나리오64회도 retry0로 통과했다. 새 입력 경로는 실제 native modal로 막히며 callback을 늦추는 임의 sleep은 넣지 않았다. native trace는 첫16중14·추가64중60만 완전/잘림없음이었고 그 범위에서 Warm Up/Restart0이다. 나머지는 횟수null로 남겼다. Chromium 자체나 DevTools 직접 강제 시작 경합을 수정했다고 주장하지 않는다.
