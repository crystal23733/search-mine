# #83 · native 워커 재시작 경합 최소 재현

[ADR0075](../../docs/adr/0075-native-worker-restart-observation.md)·[ADR0076](../../docs/adr/0076-pointer-warmup-reproduction.md)·하위 #159/#161의 기술 스파이크다. 실제 앱·게임·Workbox 없이 같은 URL의 worker bytes를 바꾸고 두 탭에서 표준 skipWaiting/clients.claim을 사용한다. 원래 10초 load 관측을 유지한다.

루트에서 Node24·기존 frozen 설치·Playwright Chromium을 준비한 뒤 실행한다. 별도 DB·WASM·제품 서버는 필요 없다.

```powershell
pnpm install --frozen-lockfile
pnpm exec playwright install chromium
node spikes/83-native-worker-restart/repro.mjs --attempts 2
node spikes/83-native-worker-restart/repro.mjs --trigger pointer --attempts 2
```

`--attempts`는 1~8이며 PC/mobile 각각 대조군·유발군을 실행한다. `--trigger`는 기본 `cdp` 또는 명시적 `pointer`다. 각 case는 임시 loopback 서버/포트/브라우저 context를 소유하고 종료 때 정리한다. 대조군은 startWorker를 호출하지 않는다. cdp 유발군만 old activated version의 stopping 관측 때 CDP startWorker를 한 번 요청한다.

pointer 모드는 동일한 단순 fetch handler/anchor를 쓰는 입력 없는 control과, 업데이트에 링크 마우스 클릭8회를 겹치는 유발군을 비교한다. 링크 탐색은 preventDefault하며 메모리/CPU 부하나 CDP worker 시작/정지 명령을 넣지 않는다. 입력이 실제 Warm Up 요청을 유발했는지는 native trace로 확인한다. 일반 사용의 재현 확률을 보장하지 않는다. Chrome153과 격리된 최신Chrome156의 실제 대조는 [검증161](../../docs/verification/161-pointer-warmup-reproduction.md)에 기록한다.

결과는 `.tmp/sw-restart-*`의 version.json·results.json·case별 JSON에 남는다. `trigger`, `restartRequested`, `inputClicksRequested/Completed`, `restartedOldVersion`, `reloadedWithinBudget`를 따로 읽는다. 요청 성공만으로 실제 재시작이나 timeout을 추정하지 않는다. `warmUpStartRequests`는 완전하고 잘리지 않은 trace의 Purpose=Warm Up 시작 횟수이며 수집이 불완전하면 null이다. trace의 complete/truncated/phase/cleanup/issue도 확인한다. 종료0은 대조군과 준비가 정상인 수집 완료이며, 유발군 reload 성공이나 #83 해결을 뜻하지 않는다. 준비/대조 실패는 종료1이다.

기존 별도8case 관측에서는 대조4/4 reload·실제old재시작4/4·강제군reload0/4였다. 후속 실제 제품 CI의 자연 실패4건은 Warm Up 목적의 정지 중 재시작이었다. pointer 모드는 이 목적을 일반 입력으로 분리해 조사한다. 설치된 Chrome의 version/revision은 실행 자료로 확인하며 다른 환경에서 동일 결과를 보장하지 않는다. 실제 제품의 각 상위 입력/heuristic과 안전한 복구는 별도이며 제품 수정이 아니다. native trace 수집 자체도 실행 시점에 영향을 줄 수 있다. #83은 OPEN으로 유지한다.

실제 제품 두 탭 E2E의 추가 native 관측은 `LIAR_TRACE_SW_NATIVE=1`로 켠다. 그 경로는 worker 시작/정지/debugger 명령을 보내지 않고, 원래 timeout·retry·network manager를 유지한다. 자료는 `.tmp/native-worker-traces`와 browser-failures artifact에 보존한다.
