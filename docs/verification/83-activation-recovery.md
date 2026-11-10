# #83 워커 활성화 복구 대조

FR10 · NFR01/02/05/06/07/08 · TS18/20/26/30 · [ADR0018](../adr/0018-public-offline-cache-and-safe-update.md)·[ADR0076](../adr/0076-pointer-warmup-reproduction.md). 현재 develop20dcda6, 기존 #83/승인된 `spikes/83-native-worker-restart` 범위다.

## 현재 근거와 한계

PR222 최신 실패의 native trace3개는 모두0events/completion_pending/finish_unavailable였다. load10초 실패 지점은 확인했지만 이번 경우의 native 목적을 추정하지 않는다. 이전 #159/161의 완전 trace와 구분한다.

수정하지 않은 기존 pointer CLI를 현재 Chrome에서 실행한 대조는 PC/mobile2건 모두 reload, 입력2건 모두 Warm Up·old재시작·원래10초 load 실패였다. 전체 원문 `.tmp/offline83-pointer-baseline.log`·고유 `.tmp/sw-restart-uwH2TP`를 보존했다. native 완료/잘림없음이며 입력8번·CDP start/stop0이다.

정확 revision의 SkipWaiting은 이미 pending 요청이 있으면 동일 호출만으로 ActivateWaitingVersionWhenReady를 다시 호출하지 않는다. old 시작은 renderer idle/asap 상태를 초기화한다. 이는 후보 선택의 코드 근거이며 실행 중인 native 변수 값을 관측했다고 주장하지 않는다.

## 다음 시나리오와 대조 계획

기존 대조/입력·버전/10초 관측/서버/브라우저 cleanup은 그대로 두고 명시적 진단 옵션으로만 후보를 분리한다. 제품 경로·기존 timeout/retry/브라우저 flags·의존성은 변경하지 않는다.

1. 업데이트 준비 잠금이 있는 모든 idle 탭의 링크 입력을 HTML inert로 막는 후보: 기존 anchor 탐색 취소만으로는 Warm Up이 남는 대조와, 제어 대상의 잠금 뒤 실제 링크 좌표 입력을 비교한다. 강제 CDP 시작/정지 없이 Warm Up/old재시작/두 탭 실제 reload를 기록한다. 이미 큐에 있던 입력과 이후 입력은 구분한다.
2. 표준 active-worker 메시지의 제한된 수명/ACK를 함께 사용하는 후보는 이전 워커의 실행과 skipWaiting 순서를 대조한다. pending lease를 새 활성화까지 기다려 교착시키지 않는다. 제품 채택 전 busy/실패/새 탭/수명 종료를 별도 검증해야 한다.
3. 단순 skipWaiting 중복 호출·동일 bytes update 같은 복구 후보는 실제 작동한 경우에만 확대한다. 무한 polling/활성화 강제·등록 해제·캐시 삭제/URL변경/페이지 강제 탐색으로 성공을 만들지 않는다.

진단 성공은 #83 제품 수정이 아니다. 유효 후보가 나오면 설계/시나리오→실제 행동 Red→Green→기존 production 반복/전체CI와 실제 diff 리뷰를 마친 뒤에만 완료를 판단한다. 실패 후보/불완전 trace를 보존한다.

## 제품 수정과 실제 Red → Green

[ADR0102](../adr/0102-update-interaction-barrier.md)는 `ActivityPort`의 prepare 잠금에 동기 구독하는 native dialog port와 OfflinePort의 준비 ACK gate를 연결한다. busy 게임은 거절하고 장벽 API 실패는 NACK+원래 token 해제다.8언어 상태·ESC 임의 해제 방지·release/기존15초 lease/dispose·유효 focus 복원을 확인한다. worker/update coordinator/activity 원본·기존2/5/8/15초 예산·브라우저 flags/retry는 변경하지 않는다.

| 검사 | 변경 전 | 수정 후 |
|---|---|---|
| 일반 production 두탭 |16PASS42.4초(실패 재현 아님)| 기존/입력 확대64회에 포함 |
| 실제 앱의 링크 입력 겹침 |8중4PASS/4load10초FAIL38.9초| 최초8/8PASS, 확대32/32PASS |
| ACK 직후 실제 UI 장벽·focus·8언어 |PC/mobile2FAIL(ACKtrue·모달없음)|8회 모두PASS(각8locale) |
| 기존 두탭/게임 거절·캐시삭제 | 위 일반 시나리오 | 최초8/8·확대32/32PASS |
| 전체 단위 | 장벽/ACK 실제Red3FAIL5PASS | targeted8PASS·전체464PASS/line91.88% |

제품 첫 Green은24PASS45.6초, 확대64PASS2.3분이며 retry0/workers2/원래load10초를 유지했다. 실제 UI 장벽 시험은 준비 메시지만 합성해 real controller source·MessageChannel로 port/화면 계약을 검사한다. 입력 겹침/기존 두탭은 실제 worker 메시지·원래 protocol을 사용하며 관측 fixture는 원래 Promise를 유지한다.

원본 실패4개는 test trace의 origin과 native Script origin을 짝지어 각각 complete/truncatedfalse·Warm Up1·old Restart1을 확인했다. `.tmp/offline83-red-paired-evidence.json`과 원본을 보존한다. 수정 첫 native16중14·확대64중60이 완전/잘림없음이고 해당 범위에서 Warm Up/Restart0이다. 불완전2/4건의 횟수는0으로 추정하지 않고null로 남긴다. 실제 두 탭 reload 성공과 native 자료 수집 성공을 구분한다.

최소 native modal/primed-modal은 각control4/4·입력4/4 reload였다. 기존 기본 CDP 모드 회귀는 control2/2 reload·직접 old재시작2/2·유발군0/2load로 기존 browser 경합을 유지했다. 이 음성 대조는 앱 입력 경로의 수정과 Chromium 자체의 수정을 구분하며 제품 성공 수에 합산하지 않는다.

타입/lint/형식/build·CLI명시lint/구문·문서270/Mermaid34/diff를 확인했다. 전체 CI와 정확한 head의 실제 코드 리뷰·병합/이슈 종료는 PR에서 추가 확인한다. 실제 PWA OS 설치/OAuth 복귀·운영 환경은 #223/#42이며 이번 로컬 검사로 완료하지 않는다.
