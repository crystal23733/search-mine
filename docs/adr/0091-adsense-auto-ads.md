# ADR0091 · AdSense 자동 광고와 문서 경계

- 상태: 채택 · #196 / 상위22 · FR12/13·NFR01/02/05/07/08
- 사용자 근거: 2026-11-05 “자동광고되게끔 구현하도록 할거야. 광고스크립트도 결국 나중에 진짜 API로 갈아끼우면 되니까”
- 선행: #194/PR195 MERGED/CLOSED / develop5fd90eb
- 대체: [ADR0090](0090-ad-request-policy.md)의 수동 전면/3판 빈도 구현 계획
- 시나리오: TS18/20/23/24/25/30

## 선택과 범위

기본 광고를 Google AdSense Auto ads로 바꾼다. Google이 배치·광고내용·노출 가능 여부를 결정하고 서비스는 스크립트 실행 가능한 문서와 현재 동의를 제어한다. 자체3판 cap·서버 광고 빈도 카운터·다중 탭 slot lock·수동 결과 전면/H5 adBreak 연동은 기본 범위에서 제거한다. #194의 미사용 Rust/TS/WASM 정책 API·광고 전용 fixture/test는 후속 제품 이슈에서 제거하고 당시 검증 기록은 보존한다. 광고의 위치·빈도를 다른 수작업 규칙으로 다시 구현하지 않는다.

애드센스는 일반 API키 대신 게시자ID와 승인된 사이트·계정 설정을 사용한다. AutoAdsScriptPort가 configured/load(signal) 경계를 제공하고 기본 미설정 adapter는 스크립트/네트워크/저장/노출을 만들지 않는다. 실제 adapter는 고정 Google HTTPS 주소와 엄격한 ca-pub 게시자ID만 사용한다. 임의 script URL·inline code/예시ID를 runtime 기본값으로 제공하지 않는다. 테스트용 광고 script/가짜CMP는 테스트에서만 DI한다.

## 허용 문서와 동의

초기 허용 문서는 유효8locale의 홈과 규칙뿐이다. query/미허용hash가 없는 공개 문서(고정 접근성 #main은 허용), 온라인·visible·현재 valid advertising 동의, 인증 초기 확인 완료·게임/activity 비사용, 홈의 첫 방문 tutorial 전환이 없는 경우에만 시작한다. 게임/튜토리얼·데일리/매칭/친구/순위·개인결과·내정보·계정/인증·설정/오류 문서에는 script를 넣지 않는다. 대상 확대는 실제 정책/화면 검수 뒤 별도 설계한다. 언어/실제 광고 배치는 Google 설정에 맡긴다. 광고 거부·미설정·차단·실패는 게임에 오류를 전파하거나 광고 대기를 요구하지 않는다.

현재 ConsentPort가 CMP의 owner/context/revision/만료를 검사하는 구조를 재사용한다. 분석 목적은 광고를 허용하지 않는다. script load는 실제 impression으로 집계하지 않으며 자동 광고의 가짜 callback/노출 이벤트를 만들지 않는다. actual CMP 없는 실행 앱은 계속 광고0이다.

## 실행 수명과 화면 전환

외부 SDK는 script 노드 제거만으로 실행이 종료되지 않는다. 문서당 단1회 load를 시작하기 **전에** 문서를 SDK 실행 가능 상태로 표시한다. load중/성공/실패 모두 이후 보호 경계를 동일하게 적용한다. load는 bounded(3초)·AbortSignal로 listener/timer/소유script를 정리하고 같은 문서의 자동 retry는 하지 않는다.

SDK가 시작된 문서에서 허용 범위 밖으로 이동할 때 router는 pushState나 App 구독자 통지 **전에** 해당 same-origin URL로 전체 문서 navigation을 시작한다. popstate도 보호 App 상태를 publish하기 전에 reload한다. navigation 요청에 실패하면 기존 안전한 화면을 유지하고 게임을 시작하지 않는다. 광고 없는 문서는 기존 SPA navigation을 유지한다. 일반 anchor의 새 문서 이동도 새 문서의 동일 gate로 검사한다.

광고 script가 시작된 뒤 동의 철회/오류/만료·auth identity/revision 변경·오프라인·활성 게임·BFCache 복원에서 허용 조건이 사라지면 요청을 취소하고 현재 문서를 한 번 reload해 SDK 문서를 폐기한다. hidden 상태에서는 새로운 load를 시작하지 않으며 복귀 때 재검사한다. 초기 game 문서와 복원된 문서에서 gate를 다시 검사한다. HMR/dispose는 구독/타이머를 정리한다. 이는 브라우저가 문서 이동을 완료하는 수명 경계이며 이미 실행된 Google script의 내부 요청을 임의 취소한다고 주장하지 않는다. 실제 CMP/TCF·운영 SDK 검수는 #147에서 수행한다.

## 운영 연결

[자동 광고 설정](https://support.google.com/adsense/answer/9261307?hl=en)과 [광고 설정](https://support.google.com/adsense/answer/9305577?hl=en)을 사용한다. 운영자는 승인된 사이트에서 Auto ads를 켜고 초기에는 **인페이지 광고만** 허용한다. 앵커/전면/vignette/사이드레일·ad intents는 꺼서 게임으로 이동하는 순간의 오버레이 개입을 제외한다. 게임/개인/인증/오류 경로의 page exclusions도 적용하고8locale·PC/mobile·직접 진입/뒤로가기를 실검수한다. [제외 URL](https://support.google.com/adsense/answer/9262311?hl=en)은 query/hash를 지원하지 않으므로 code gate도 이들을 제외한다. [영역 제외](https://support.google.com/adsense/answer/12626543?hl=en)는 overlay 차단을 대체하지 않는다.

게시자ID·승인·CMP factory·CSP script/frame/connect allowlist·실제 ads.txt는 #147/21/22/24의 운영 입력/검수다. wildcard/unsafe-inline으로 통과시키지 않는다. publisher ID는 공개 설정이지만 계정 비밀값·예시 ID는 배포하지 않는다. 테스트 script 성공으로 Google 승인/실광고/수익을 완료로 보고하지 않는다.

## 순서와 검증

승인 근거/요구/시나리오를 담은 #196 PR을 develop에 병합·CLOSED 확인 뒤 제품 이슈에서 구현한다. 사용자 지시로 자동 광고 변경과 기존 리뷰·병합 위임이 있으므로 반복 승인 대기는 하지 않는다.

제품 TDD는 기본 외부요청0·허용 문서/8locale·동의/온라인/visible/첫방문·스크립트 단1회/timeout/취소/실패·게임 navigation 전 barrier·popstate/BFCache·철회/계정/오프라인 reload의 행동Red→Green을 확인한다. 실제 브라우저에서 테스트 script를 네트워크 경계로 주입해 safe 문서의1회 실행과 게임 새 문서의0실행·locale/뒤로가기·차단/timeout 무차단을 검증한다. Rust 원천 삭제 뒤 타입/WASM/기존 native fixture를 재생성/검사하고 전체회귀/coverage·문서/Mermaid·정확head 리뷰/6checks 후 병합한다. #83의 timer/flags/retry/성능 목표는 바꾸지 않는다.

## 2026-11-13 승인된 Google CMP bootstrap 보완

[ADR0107](0107-google-cmp-bootstrap.md)과 사용자 명시 선택은 Google 공식 메시지 bootstrap에 한해 태그를 공개 문서에서 먼저 로드하고 광고 요청을pause하는 경로를 승인한다. 이 문서의 동의 후script시작 조건은 Google모드에서 “현재 광고 동의 후 요청 재개”로 보완한다. 일반미설정/대체adapter, 게임/개인/인증 문서 격리와 거부/실패 광고0은 유지한다.
