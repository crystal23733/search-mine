# #161 · 일반 링크 입력 Warm Up 최소 재현

FR10·NFR01/02/05/07/08 · TS18/20/26/30 · [ADR0076](../adr/0076-pointer-warmup-reproduction.md). 선행 #159/PR160 병합·CLOSED와 최신 developd43b91e clean/원격 일치를 확인했다. 구현보다 먼저 ADR·시나리오·추적표·백로그를 갱신한다.

기존 자연 실패 source3fbaa8f2건과 ed251172건은 모두 old 정지 중 Warm Up 시작→Restart=true→old 재시작·boot1/waitinginstalled였다. ed의 old waitUntil107/107·respond40/40·handled41/41 완료와 new skipWaiting1/0도 짝지어 확인했다. `.tmp/native83-ed-natural-summary.json`에 최신 두 건을 보존했다. 동일 ed의 Product39044508195 attempt2는119/9.4분/flaky0·native2완료였고, attempt1 실패116/1/2·8.7분 및PWtrace2/native11을 별도로 유지한다. #159는 도구의 완료이며 #83은 OPEN이다.

추가 ignored prototype `.tmp/sw-input-warm-EPdzSS`는 앱/Workbox 없이 fetch/anchor·8/16/32/64 실제 클릭을 업데이트와 겹쳤다. PC/mobile16case에서 control8/8 reload·입력군8/8 old재시작/원래10초timeout, 실제 Warm Up 목적을 관측했다. 이 실험에는100000객체 heap이 있으므로 제거한 비교와 최종 CLI 결과를 구분한다. 처음 사례의 Warm Up 요청은 old stop 약1.44ms 뒤, OnStopped Restart=true 약0.23ms 전이었다. CDP startWorker를 호출하지 않았다.

별도 최신 Playwright1.64.1/Chrome156.0.8078.4는 ignored 독립 package/store/browser에서 동일 ed 제품 시나리오8개가24.7초/retries0 통과했다. root 의존성/lockfile은 변경하지 않았다. native8중6완료/2completion_pending, 모두cleanupdetached/truncatedfalse였다. 이는 전체119/실DB나 원인 수정의 증거가 아니다. `.tmp/native83-latest-comparison-evidence.json`과 source-manifest를 보존했다.

heap 없는 비교 `.tmp/sw-input-noheap-LsD4cz`도 control4/4 reload·입력군4/4 old재시작/10초timeout이었다. 최종 도구에는 heap을 넣지 않았다. 새 `--trigger pointer`의 기존 CLI Red는 exit1/Unknown option이었고, 구현 후 Green은 exit0/대조 정상·유발 결과 보존이다. 이는 제품 load 실패를 통과로 바꾼 것이 아니다.

| 실행 | control reload | 유발군 CDP 요청 | 실제 old재시작 | 유발군10초 reload | native 완료 |
|---|---:|---:|---:|---:|---:|
| 최종 pointer · PW1.63/Chrome153 ·8case | 4/4 | 0/4 | 4/4 | 0/4 | 7/8 |
| 기본 cdp 회귀 · PW1.63/Chrome153 ·4case | 2/2 | 2/2 | 1/2 | 1/2 | 4/4 |
| 동일 pointer · 격리 PW1.64.1/Chrome156 ·4case | 2/2 | 0/2 | 2/2 | 0/2 | 4/4 |

최종 pointer는 각 유발군8클릭을 모두 완료했다. Chrome153의 Warm Up 요청은 old stop 약0.17~1.49ms 뒤, Chrome156은0.41/1.72ms 뒤에 들어왔고 모두 OnStopped Restart=true 전이었다. 유발군 native6개가 모두완료/잘림없음이며 waitinginstalled/boot1과 일치했다. 기본 cdp는 요청2건 중 실제 재시작1건만10초timeout이고 재시작되지 않은1건은reload됐다. 요청과 관측을 동일시하지 않는다. 누락된 pointer control1개의 Warm Up 횟수는null, 모든16case cleanup은detached였다.

이전 제품 시나리오8통과와 달리 최신 Chrome156에서도 이 최소 경합은 관측됐다. 따라서 버전 교체만으로 #83이 해결된다고 하지 않는다. 실행 자료는 `.tmp/sw-restart-oviZ6F`·`.tmp/sw-restart-ob7k7k`·`.tmp/sw-restart-zLul5l`이며 CLI hash/정확 브라우저revision/종료값/정지 구간은 `.tmp/native161-local-evidence.json`에 보존했다. 루트 의존성/lockfile은 불변이다.

기존 collector8개/11.04초·프로젝트 lint/typecheck·스파이크 명시적 lint/구문·MJS형식·문서210·Mermaid34오류0을 실제 확인했다. backlog.json은 기존 항목의 형식 변경을 되돌리고161항목만 추가했으며JSON내용을 대조했다. 코드 리뷰는 모드 검증/반복 상한·입력/control 동일fixture·preventDefault·CDP 시작 guard·수집 불완전null·Promise/서버/context 정리·제품/비밀 변경 부재를 확인했다.

원격 필수checks/정확head/CLEAN·병합 상태는 실행 뒤 PR과 당일 `/review`에 기록한다. 원자료는 ignored `.tmp`에 보존한다. #83 제품 복구는 미완료다.
