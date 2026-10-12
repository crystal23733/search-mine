# ADR0061 · 로그인 없는 데일리 두 순위 화면

- 상태: 채택 · M3/#130, 상위 #19
- 선행: #128/PR129 develop652e6c8 병합·#128 CLOSED 실제 확인
- 요구: FR09/10/11, NFR01/02/05/06/08, TS17/18/19/20/21/26/36

## 결정

기존 로컬 데일리의 명시적 순위 링크에서 `/{locale}/daily/rankings`로 진입한다. 이 화면은 게임 core를 초기화하지 않는 lazy 화면이며 공개 순위 읽기 포트만 사용한다. 기존 DailyHttp의 closed Rust→TS DTO·credentials:omit·무CSRF·64KiB/2slot/10초 경계를 재사용한다. 앱의 읽기 전용 guest bootstrap이 서버 계정을 만들지 않는 기존 계약을 유지한다. 제출·공식 시작·기존 pending flush를 호출하지 않는다.

공개 순위 요청의 무쿠키 계약과 전체 브라우저의 쿠키 부재를 구분한다. 기존 guest bootstrap은 브라우저 바인딩용 nonce 쿠키를 발급할 수 있으며 세션/계정 생성과 다르다. 순위 요청에는 이 쿠키도 전송하지 않는다. 실제 익명 출구는 순위 요청 Cookie/CSRF 부재·세션 쿠키 부재와 독립 bootstrap의 account/session_revision:null로 검사한다.

UTC wall clock의 날짜와 DAILY_SEED_VERSION을 조회에 사용하고 그 날짜를 상태에 고정한다. 명시 새로고침은 현재 UTC 날짜를 다시 읽는다. 새 요청은 이전 요청을 취소하고 이전 rows를 지운다. request identity와 dispose/abort를 확인해 역순 또는 늦은 응답을 폐기한다. locale 변경은 같은 상태를 번역하며 게임·조회 상태를 새로 만들지 않는다. offline이면 rows/진행 요청을 제거하고 조회를 차단한다. 연결 복구의 공개 읽기는 허용하되 pending 제출과 연결하지 않는다.

화면 이탈의 controller dispose는 layout effect cleanup으로 commit 시점에 실행한다. 설치된 Preact11의 passive effect cleanup은 paint 뒤로 지연되므로 즉시 abort를 보장하지 않았다. DOM 이탈 테스트의 실제 실패를 보존하고 cleanup 시점을 보정한다. 서버 요청과 상태 구독은 기존 effect를 사용하며 게임 로직을 layout effect에 넣지 않는다.

서버 `metadata:null`은 미발행으로 표시한다. metadata가 있는 정상 빈 두 목록, loading, HTTP 실패와 offline을 구분한다. 실패를 빈 성공으로 표시하지 않는다. 서버의 time과 completion 두 목록을 별도 semantic table/caption/column header로 표시하고, position·현재 nickname·mistakes·서버 elapsed만 사용한다. 시간 표시는 밀리초를 유지해 다른 서버 값을 같은 정밀도로 뭉개지 않는다. client elapsed/개인 기록을 공식 시간으로 승격하지 않는다. 각50개/has_more는 상위 목록 안내로 표시한다. 공개 rows에는 account ID가 없으므로 nickname으로 본인 행이나 본인 순위를 추정하지 않는다. metadata의 seed/hash와 내부 식별자를 화면·URL·새 저장소에 추가하지 않는다.

제공 디자인11/12의 순위 진입점·카드·공통 Button/NavLink/토큰을 재사용한다. 8언어와 좁은 화면의 긴 닉네임·키보드·semantic table·상태 안내를 검증한다. 광고와 계정 요구를 추가하지 않는다.

## 검증과 후속

실제 HTTPS의 기존 미구현 경로에서 순위 제목 부재를 행동 Red로 확인한다. fake port/clock으로 UTC 새로고침·역순/취소/dispose·offline/복구·실패와 미발행/빈 결과를 검사한다. DOM에서 서버 position/같은 nickname/밀리초·50개/has_more·실패 시 이전 rows 제거를 검사한다. 실제 WASM replay→서버 ticket/heartbeat→PostgreSQL 저장 뒤 익명 HTTPS의 두 순위·8언어 PC/mobile·무쿠키/무CSRF·503 재시도·offline 경계를 검사한다. full/관련95·전체80/문서/Mermaid/코드 리뷰/최신CI 후 병합·CLOSED를 확인한다.

공식 시작/controller/heartbeat 화면·본인 receipt·검증 결과 공유와 #19 전체 통합 출구는 다음 필수 작업이다. 이 순위 화면만으로 #19를 종료하지 않는다. OAuth42·운영25/26·사람27·미확정83은 별도다. 추가 의존성·DB migration·제품 규칙 변경은 없다.
