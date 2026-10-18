# ADR0076 · 링크 입력으로 유발하는 Warm Up 재시작 재현

- 상태: 채택 · 승인된 FR10·NFR01/02/05/07/08 · #161, 상위 #83
- 선행: #159 CLOSED·PR160/developd43b91e 병합
- 시나리오: TS18/20/26/30

## 근거

#159의 자연 CI 실패4건에서 old activated worker 정지 중 Purpose=Warm Up 요청과 OnStopped Restart=true·old 재시작·waiting 미완료를 관측했다. source3fbaa8f의2건과 sourceed25117의2건은 원래10초 load에 최초·기본 재시도 모두 실패했고 원자료를 각각 보존했다. ed의 요청은 정지 시작 약5.75ms/12.03ms 뒤였다. 반면 동일 ed의 실패 web job만 한 번 재실행한 결과는119개 통과였다. 이 성공은 경합의 수정 근거가 아니다.

앱/Workbox 없는 추가 실험은 fetch를 원래 네트워크로 전달하는 worker와 preventDefault하는 일반 anchor를 사용했다. 업데이트와8~64회의 실제 마우스 클릭을 겹쳤다. 인위적인100000객체 heap이 있는 첫16case는 control8/8 reload·입력군8/8 old재시작/10초timeout이었다. heap 없는 별도8case도 control4/4 reload·입력군4/4 old재시작/10초timeout이었다. stop/start CDP 명령은 보내지 않았다. 입력이 Warm Up 목적을 유발하는 최소 경로를 분리하며 실제 앱의 각 상위 입력까지 확정하지 않는다.

설치 Chrome153 revision의 [anchor interaction](https://raw.githubusercontent.com/chromium/chromium/971a7443b0c9b0a9b2860529b33331b76077ec62/content/browser/preloading/anchor_element_interaction_host_impl.cc)은 pointerdown/hover에서 워밍업을 요청한다. [ContextWrapper](https://raw.githubusercontent.com/chromium/chromium/971a7443b0c9b0a9b2860529b33331b76077ec62/content/browser/service_worker/service_worker_context_wrapper.cc)는 running active를 건너뛰지만 stopping의 시작 대기를 막지 않는다. 정확 revision 소스와 실제 trace를 함께 사용하며, 소스만으로 실행 콜스택을 추정하지 않는다.

## 결정

승인된 `spikes/83-native-worker-restart`의 진단 CLI에 명시적 `--trigger pointer`를 추가한다. 기존 기본값 `cdp`와 `--attempts 1~8`을 유지한다. 두 모드 모두 PC/mobile의 입력 없는 control과 유발군을 순서대로 실행한다. pointer의 두 군은 동일한 fetch/anchor fixture를 쓰고, 유발군만 둘째 탭의 anchor에8회 마우스 클릭을 업데이트와 겹친다. anchor의 기본 탐색을 취소해 별도 페이지 탐색으로 reload 성공을 만들지 않는다. 인위적인 heap/CPU 부하는 넣지 않는다.

결과에 trigger=control/cdp/pointer, 입력 요청/완료 횟수·CDP 재시작 요청 여부·실제 old재시작·원래10초 reload를 분리한다. Warm Up 목적 시작 횟수는 native trace가 complete이고 truncated=false일 때만 숫자로 기록하며 불완전 수집은 null이다. 실제 old재시작 여부는 별도 version 관측으로 계산한다. complete/cleanup/phase/issue와 version/revision·원자료를 보존한다. 일반 입력 모드는 CDP ServiceWorker.startWorker/stopWorker를 보내지 않는다.

종료0은 control/준비 정상과 수집 완료이며 유발군 성공이나 제품 수정 완료가 아니다. 재현 성공률을 보장하지 않는다. 제품 SW/업데이트/게임·규칙·load10초/준비 예산·retry/network manager·의존성은 변경하지 않는다. 안전한 복구는 #83의 후속이다. 외부 프로젝트에 게시/연락하지 않는다.

## 검증

새 CLI 옵션을 기존 도구에 전달한 실제 실패 Red를 보존한 뒤, PC/mobile의 두 모드·control/입력/old재시작/native 목적/10초 결과로 Green을 검토한다. 기본 CDP 모드도 회귀 실행한다. 기존 collector8개·형식/lint/typecheck·문서/Mermaid·필수 CI와 정확head/CLEAN을 확인한다. 스파이크의 유발군 timeout은 의도한 관측이며 제품 E2E 통과 수에 더하지 않는다.
