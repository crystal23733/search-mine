# #156 · 동의 컨텍스트·만료 검증

FR01/10/11/13 · NFR01/02/05/06/07/08 · TS18/19/20/24/26/29/31/35/36 · [ADR0073](../adr/0073-consent-context-and-expiry.md).

선행 #20/PR146·#152/PR153·#154/PR155 병합/CLOSED 확인. 코드보다 먼저 설계·시나리오·추적표·백로그를 갱신한다. 제어된 시간/revision/context와 실제 DOM·PC/mobile/관련 전체 검증을 수행 뒤 기록한다. 실제 CMP·서버 동의·선택 수집·외부147/상위21과83/141은 별도 미완료다.

## 행동 검증

초기 passthrough 후보의26개는3pass/23fail Red였고 현재 receipt/만료/revision/clock/context/abort 구현 뒤26/26 Green이었다. 초기 기대 배열의 정렬 오류와 sandbox의Vite spawn EPERM은 제품 Red에 합산하지 않았다. 브라우저 컨텍스트5개·부분 구독 정리1개의 추가 Red를 확인했다. 직접 guard read 오류의 pending 취소도29pass/1fail Red→보정했고,8언어 실제 DOM/만료 화면을 포함한 관련61개가 통과했다.

추가 malformed/source 실패·동일 revision 기간 변경·pre-abort/unknown context·동기 구독 실패를 포함한 guard 집중40개가 통과했고 측정 당시121/127lines·95.27%였다. 초기 웹 전체355개/91.43% 측정 뒤 의미 있는 경계 시나리오를 추가했다. 타입/lint/format·문서204/Mermaid34오류0을 확인했다. 아직 완료되지 않은 최신 전체/CI 결과는 PR과 당일 review에 별도로 기록한다.

실제 PC/mobile storage 시나리오6개는14.1초·retries0·workers2로 통과했다. 두 탭의 실제 BroadcastChannel에서 위조 choice와 invalidate가 probe에 전달된 것을 확인한 뒤 실행 앱의unavailable/양쪽차단/재열기disabled와실제WASM256셀연습을검사했다.8언어고지·저장소거부/503에서도선택요청0·게임무차단을유지했다. 실제 CMP의 성공으로 해석하지 않는다.

## 코드 리뷰

메모리 token 참조 동일성·검증 시작 시 캡처·revision floor/같은 revision 변조·wall/monotonic deadline·낮은 revision 비복원을 검토했다. 부분 구독 실패 시 이미 붙인 observer/timer를 정리하고 현재 read 예외도 진행 중 adapter를 취소한다. caller와 context의 취소는 adapter가 무시해도 Promise를 해제한다. 소스/컨텍스트·브라우저 계정·UI 책임을 분리했다.

어댑터 미설정 조립은 추가 채널/계정 구독도 만들지 않는다. peer는 invalidate만 전달하고 선택/계정/token/credential을 저장·전송하지 않는다. 실제 AuthPort의ready는 서버 저장 proof가 아니므로 선택 수집/광고 요청을 켜지 않는다. 실제 factory의 CMP/TCF·지역·서버 transaction 연결과 #147 검수는 필수 후속이다. `/review`와 원자료는 Git에서 제외한다.

## 최초 후보와 후속 리뷰 보정

최초 source `f1994a2839c7b320b060f0dfbfb108c81500cc4a`의 새 로컬 DB 전체는 Rust430·웹362/91.54%·진단8 통과 뒤 기존 #83 mobile 두 탭 load10초 실패로 Chromium116pass/1fail/2미실행·5.4분이었다. trace1/probe2·원본 로그/exit·DB를 `.tmp/consent156-first-full-failure`에 보존했다. 이전 worker의 respond40/40·handled41/41 완료와 새 skipWaiting 미완료를 관측했지만 body/native idle·본 원인은 미확정이다. 별도 기본8/30.6초·계측비활성 비교8/28.5초는 retries0으로 통과했고, 비교용16probe를 보존했다. 최초 실패와 합산하지 않는다.

같은 source의 Product38997751007은 SUCCESS였다. Rust430·웹362/91.54%·진단8·Chromium119/7.8분/flaky0·실PG 일반150/61.14초·계측151/60.89초를 확인했고 job 로그4종·artifact4종을 `.tmp/consent156-ci-first-f1994a2`에 내려받았다. 이 성공은 이후 리뷰 보정의 검증으로 사용하지 않는다.

코드 리뷰에서 adapter.read/receipt getter 실행 중 컨텍스트가 바뀌거나 시간이 만료되는 경계를 발견했다. 세 시나리오의 실제 Red40pass/3fail을 확인한 뒤 receipt 필드를 읽은 후 컨텍스트·시각을 다시 검사하도록 보정했고 관련71개가 통과했다. 타입/lint/format·문서204/Mermaid34오류0도 통과했다. 이 보정을 포함한 최신 head의 전체/CI 결과는 완료 후 PR과 당일 review에 구분해 기록한다.
