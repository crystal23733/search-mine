# ADR0075 · native 워커 재시작 최소 재현과 관측 보존

- 상태: 채택 · 승인된 FR10·NFR01/02/05/07/08 · #159, 상위 #83
- 선행: #141 CLOSED·PR158/develop796962b 병합
- 시나리오: TS18/20/26/30

## 근거와 판단 범위

자연 실패는 두 idle ACK·JS waitUntil/respondWith/handled 완료 뒤 old worker가 정지·재시작하고 waiting의 skipWaiting이 완료되지 않는 상태다. 별도 실제 앱 대조16회·PWtrace 추가16회·native observer 전체119개는 통과했지만, old activated version의 stopping 때 CDP startWorker를 강제한8회는 모두 원래10초 load 실패였다. 앱/게임/Workbox 없는 최소 예제도 제어권 준비를 바로잡은8회에서 control4/4 reload·실제old재시작4/4·강제군reload0/4였다. 초기 준비 오류·ENOSPC 실행은 집계에서 제외했다.

추가1회는 원래10초 실패를 유지한 뒤30초 더 관측했고, 재시작 약30초 뒤 native RequestTermination이 will_be_terminated=false였으며 boot1/waitinginstalled가 유지됐다. 실제 Chrome153.0.8010.12/revision971a7443b0c9b0a9b2860529b33331b76077ec62의 자료를 보존했다. 이는 유발 조건의 관측이다. 자연 실패의 자동 재시작 요청자와 설치 revision의 내부 분기는 아직 미확정이며 #83은 OPEN이다.

## 결정

기술 스파이크를 `spikes/83-native-worker-restart`에 둔다. 기존 Playwright와 Node 표준 API만 사용하고 앱/게임/Workbox를 불러오지 않는다. 각 case가 loopback 서버·임의 포트·메모리 revision·두 임시 탭을 소유한다. 같은 worker URL의 bytes만 바꾸고 첫 controller 준비 후 둘째 탭을 연다. bounded CLI 반복과 browser version/revision·재시작 요청/실제old재시작·원래10초 reload·native trace를 `.tmp`에 남긴다. 강제군 timeout을 제품 실패 수정/성공으로 바꾸지 않으며 종료0은 대조군이 정상인 수집 완료를 뜻한다.

실제 두 탭 E2E에는 `LIAR_TRACE_SW_NATIVE`가 있을 때만 Browser CDP의 ServiceWorker category trace를 추가한다. CI web은 이 관측을 켜고 기존 network manager·제품/테스트 timeout·retry는 유지한다. worker debugger·stop/start/활성화 강제는 자연 관측 경로에 넣지 않는다. trace는 격리 fixture의 전용 브라우저에서만 수집하고 `.tmp/native-worker-traces`와 기존 browser artifact에 보존한다. 제품 산출물에는 포함하지 않는다.

collector는 시작 ACK를 받은 자신의 trace만 종료한다. 다른 trace가 이미 시작돼 거절되면 그것을 종료하지 않고 자신의 세션/리스너만 정리한다. 완료 리스너를 End보다 먼저 붙여 동기 완료도 보존한다. 최초1초 관측 예산은 Product39042067694의 Linux에서 두 trace 모두0event/finish_unavailable로 끝났다(제품119개는 첫 실행 통과). 이 자료를 보존하고 진단 종료에만5초 상한을 적용하며 end_pending/end_rejected/completion_pending/complete 단계를 구분한다. 제품·인수 테스트의 timeout/retry는 그대로다. 중복 종료는 합치며 최대50000event·잘린 여부/완료/실패를 명시한다. 예외 원문은 진단 상태에 복사하지 않는다. 수집은 관측 시점에 영향을 줄 수 있으므로 성공을 원인 해결로 해석하지 않는다.

## 검증

종료 예산 뒤 도착한 End 응답/실패·trace event는 이미 반환한 닫힌 종료 단계/자료를 바꾸지 못하게 한다. 실제 누락/멈춤 test의 late completion으로 확인한다.

설치 revision의 Tracing handler가 완료 event에 제공하는 dataLossOccurred도 truncated에 반영한다. 50000개 상한 미달이어도 브라우저가 손실을 보고하면 온전한 자료로 해석하지 않는다. 종료 완료와 자료 손실은 별개다.

5초·단계 보정 뒤 로컬8회도 제품8통과/native7완료·1 completion_pending/0event였다. 대기 상한만으로 관측 완료를 주장하지 않는다. 페이지 CDP 구독 세션 detach 전에 소유 Browser trace의 종료·자료 보존을 마치는 순서로 보정하고 실제 대조한다. 관측 경로 순서의 조사이며 #83 제품 원인 확정은 아니다.

가짜 CDP 경계의 소유권/동기 완료/누락 완료/오류/중복 종료/구독 정리·상한을 실제 Red→Green으로 확인한다. 실제 PC/mobile 최소 대조와 production 두 탭·전체 회귀·문서/Mermaid·코드 리뷰·최신6checks/정확head를 기록한다. 첫 실패는 재시도 성공과 별도로 남기고 #83의 제품 수정 완료는 주장하지 않는다.

근거: [ADR0018](0018-public-offline-cache-and-safe-update.md), [CDP ServiceWorker](https://chromedevtools.github.io/devtools-protocol/tot/ServiceWorker/), [CDP Tracing](https://chromedevtools.github.io/devtools-protocol/tot/Tracing/).
