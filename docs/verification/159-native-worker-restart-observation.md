# #159 · native 재시작 경합 재현·관측

FR10·NFR01/02/05/07/08 · TS18/20/26/30 · [ADR0075](../adr/0075-native-worker-restart-observation.md).

선행 #141/PR158 병합·CLOSED와 develop796962b clean/원격 일치를 확인했다. 설계·시나리오·추적표·백로그를 먼저 작성했다. 실제 앱 강제8/8 timeout, 앱 없는 최소 대조 control4/4 reload·old재시작4/4·강제군reload0/4와 추가30초 관측을 로컬 review/원자료에 보존했다. 이것을 자연 실패의 요청자 확인이나 제품 수정 완료로 확대하지 않는다. #83 OPEN을 유지한다.

collector 실제 stub Red0pass/6fail 뒤 Green6pass/0fail을 확인했다. 시작 소유권·동기 완료·부분 자료/End 오류·완료 누락/멈춘 End 각각1초·중복 종료 동일Promise·50000상한/구독 정리를 검증했다. lint/typecheck·문서208·Mermaid34오류0을 실제 실행했다.

tracked spike 첫8case는 control4/4 reload·요청/old재시작4/4·강제군reload0/4였다. 정리 경로를 보완한 최종8case는 control4/4 reload·요청4/4·실제old재시작3/4·강제군reload1/4였다. 실제old재시작3회는 모두 원래10초 load_timeout이고, 재시작 요청만 있었던1회는 reload됐다. 요청과 실제 재시작을 따로 기록해야 함을 확인했다. 모든 trace는 complete/truncated/issue와 설치 Chrome version/revision을 보존한다.

전체/CI 결과는 실행 뒤 PR과 당일 review에 기록한다. 초기 준비 오류/디스크 부족 시도와 최종 도구 결과를 구분하며 `/review`와 `.tmp`는 Git에서 제외한다. 코드 리뷰는 다른 owner의 trace 종료 금지·리스너 선등록·진단 상한·자연 관측 경로의 worker 조작 부재·기존 제품 정책/규칙 불변을 확인했다.

최초 source92d0724/Product39042067694는 Rust431·웹365/91.54%·실PG150/41.84초 및 계측151/38.17초·server96.02%·Chromium119/6.1분/첫 실행 통과였다. 로컬 전체119/5.9분과 관측off 대조8/29.4초도 통과했다. 그러나 실제 artifact의 Linux native trace2개는 모두0event/finish_unavailable였으므로 관측 인수는 미완료다. 실패 자료를 별도 보존한 뒤 ADR/시나리오를 먼저 보정하고 진단 종료5초·단계 구분의 Red→Green과 실제 CI 자료를 재검증한다. 제품/test load·native main 예산을 늘린 것이 아니다.

단계 보정 Red2pass/4fail→Green6pass, 늦은 End 응답의 결과 덮어쓰기 Red5pass/1fail→settled guard Green6pass를 실제 확인했다. 추가 product PC/mobile8/25.3초·정리 순서 보완8/25.8초는 각각8개 통과였으나 native는 각각7complete/1completion_pending0event였다. 정리 순서나 시간만으로 수집 누락 해결을 주장하지 않는다. 누락/부분 수집을 명시하는 bounded 관측과 실제 Linux 자료의 유용성을 재검토한다. 원래 자연 실패 #83과 진단 수집 한계를 별도로 남긴다.
