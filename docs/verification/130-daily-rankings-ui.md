# #130 로그인 없는 데일리 두 순위 화면

[이슈130](https://github.com/crystal23733/search-mine/issues/130) · [ADR0061](../adr/0061-daily-rankings-ui.md) · M3/상위19.

선행128/PR129 develop652e6c8 병합·128 CLOSED를 실제 확인한 뒤 설계·시나리오부터 작성했다. 로컬 데일리의 명시적 링크에서 로그인 없는 lazy 두 순위 화면으로 이동한다. strict DailyHttp 공개 읽기와 공통 토큰·카드를 재사용하며 새 계정/제출/규칙/의존성/DB migration은 추가하지 않는다.

## 행동 검증과 수정

새 PostgreSQL 테스트DB·실제 HTTPS에서 WASM replay→서버 ticket/heartbeat→verified_time 저장까지 성공한 뒤, 기존 `/en/daily/rankings`의 순위 h1 부재로 Red exit1을 확인했다. 준비 오류를 Red로 세지 않았다. 구현 후 공개 두 목록의 현재 nickname·서버 position/시간, 8언어 PC1440/mobile390, 503 실패 시 rows 제거와 명시 재시도, offline 취소/차단·복구, 문서 가로 넘침 없음, 무쿠키/무CSRF·세션/계정 부재·daily 쓰기0을 확인했다. 초점 Green 2개/9.3초/retries0.

첫 HTTPS Green의 전체 쿠키0 단언은 기존 guest bootstrap의 브라우저 바인딩 nonce를 세션과 혼동하여 실패했다. ADR0061에 계약을 명확히 하고 공개 순위 요청의 Cookie/CSRF 부재, `__Host-liar_session` 부재, 독립 bootstrap의 account/session_revision:null을 검사하도록 수정했다. 실패 로그/trace를 보존했다. 인증 쿠키/계정 요구를 제품에서 바꾸지 않았다.

단위 port/clock으로 UTC 자정 새로고침·잘못된 시각 무요청, 역순/늦은 실패/성공·교체/중단/dispose, offline/복구, 실패와 정상 미발행을 검사한다. DOM에서는 미발행/발행된 빈 목록/실패의 구분, 같은 nickname으로 본인 추정하지 않음, 서버 밀리초1234/1235를 각각0:01.234/0:01.235로 표시, 50개/has_more, 두 semantic table/column header, loading 중복 차단과 unmount 즉시 취소를 검사한다.

첫 DOM 실행의 미설치 matcher를 기본 textContent 단언으로 수정했다. unmount 즉시 abort 실패는 초기 요청 완료를 기다려도 재현됐다. 설치된 Preact11의 passive cleanup 지연을 실제 source에서 확인하고 ADR를 먼저 보강한 뒤 controller dispose만 layout cleanup으로 이동했다. 해당 DOM3개 통과. 첫 전체 검사의 jsx-a11y lint 실패는 미발행 상태를 semantic output으로 보정했고 lint/typecheck 통과했다. 각 실패 원본은 로컬에 보존하며 성공 근거와 구분한다.

## 전체 검사·리뷰

새 로컬 PostgreSQL17.4 DB liar_daily130fullb, 기존 migration, CARGO_INCREMENTAL=0으로 scripts/check.ps1을 실행 중이다. 최종 전체/coverage와 최신 원격CI·정확head 병합·CLOSED 결과는 이어서 기록한다.

correctness/changes/develop652e6c8 순차 코드 리뷰에서 shared lazy API의 수명/공개2slot·10초 경계와 기존 pending 전후 proof, UTC 갱신/이전 rows 폐기·identity/abort/dispose/offline, locale 상태 유지와 game core 비호출, strict server position/nickname/시간·두 신뢰 수준/secret 미출력, 접근성/8언어를 확인했다. 추가 재현 가능한 결함은 확인하지 못했다. 독립 보안감사·실계정·실기기·사람 관찰을 수행한 것은 아니다.

공식 시작/controller·heartbeat 화면·본인 receipt/검증 결과 공유와 #19 전체 출구는 후속이다. 외부42·운영25/26·사람27·원인 미확정83은 별도다.
