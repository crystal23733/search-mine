# #83 · native 워커 재시작 경합 최소 재현

[ADR0075](../../docs/adr/0075-native-worker-restart-observation.md)·하위 #159의 기술 스파이크다. 실제 앱·게임·Workbox 없이 같은 URL의 worker bytes를 바꾸고 두 탭에서 표준 skipWaiting/clients.claim을 사용한다. 원래 10초 load 관측을 유지한다.

루트에서 Node24·기존 frozen 설치·Playwright Chromium을 준비한 뒤 실행한다. 별도 DB·WASM·제품 서버는 필요 없다.

```powershell
pnpm install --frozen-lockfile
pnpm exec playwright install chromium
node spikes/83-native-worker-restart/repro.mjs --attempts 2
```

`--attempts`는 1~8이며 PC/mobile 각각 대조군·강제 재시작군을 실행한다. 각 case는 임시 loopback 서버/포트/브라우저 context를 소유하고 종료 때 정리한다. 대조군은 startWorker를 호출하지 않는다. 강제군만 old activated version의 stopping 관측 때 CDP startWorker를 한 번 요청한다.

결과는 `.tmp/sw-restart-*`의 version.json·results.json·case별 JSON에 남는다. `restartRequested`, `restartedOldVersion`, `reloadedWithinBudget`를 따로 읽는다. 요청 성공만으로 실제 재시작이나 timeout을 추정하지 않는다. trace의 complete/truncated/issue도 확인한다. 종료0은 대조군과 준비가 정상인 수집 완료이며, 강제군 reload 성공이나 #83 해결을 뜻하지 않는다. 준비/대조 실패는 종료1이다.

기존 별도8case 관측에서는 대조4/4 reload·실제old재시작4/4·강제군reload0/4였다. 설치된 Chrome의 version/revision은 실행 자료로 확인하며 다른 환경에서 동일 결과를 보장하지 않는다. 자연 실패의 자동 재시작 요청자는 미확정이다. 강제 재시작은 유발 조건 대조이며 제품 수정이 아니다. native trace 수집 자체도 실행 시점에 영향을 줄 수 있다. #83은 OPEN으로 유지한다.

실제 제품 두 탭 E2E의 추가 native 관측은 `LIAR_TRACE_SW_NATIVE=1`로 켠다. 그 경로는 worker 시작/정지/debugger 명령을 보내지 않고, 원래 timeout·retry·network manager를 유지한다. 자료는 `.tmp/native-worker-traces`와 browser-failures artifact에 보존한다.
