# ADR0083 · 내 정보 클라이언트와 파일 제공 경로 제거

- 상태: 채택 · #178 · 선행 #177/PR180 병합·CLOSED 확인 (develop6efec66).
- 사용자 결정: 파일 다운로드 제거, 내 정보에서 전적·계정·설정·현재 동의 확인.
- 추적: FR01/09/11/13/16 · NFR01/02/05/06/07/08 · TS17~22/24/26/29/34~36.

## 화면과 경계

`/me` 요약, `/me/history` 전적, `/me/account` 계정 관리, `/me/privacy` 개인정보·현재 동의를 제공한다. 헤더와 홈 계정 카드에서 내 정보로 이동하며, 닉네임이 없는 인증 사용자도 조회·관리를 이용한다. 기존 `/results`와 실제 대전의 닉네임 조건은 유지한다. 로그인·재인증의 복귀 대상은 고정 allowlist에 내 정보/계정 경로를 추가하고 임의 URL을 허용하지 않는다.

`/settings`는 로그인 없는 이 기기 설정으로 유지한다. 언어·모션·대비·확대와 캐시/로컬 기록·CMP 제어를 기존 port로 제공하고, 내 정보의 설정 링크도 같은 화면으로 연결한다. 계정 관리 컴포넌트는 `/me/account`로 옮겨 한 곳에서 사용한다. 계정과 기기 설정의 저장 범위를 설명하고 새 계정 설정 필드를 만들지 않는다.

전적 목록은 온라인/공식 데일리, 온라인의 모두/사람/봇 필터와 최대20개 페이지를 제공한다. 서버가 반환한 전체 요약과 total을 표시하며 클라이언트가 승패·승률·공식 시간을 다시 계산하지 않는다. 상세는 이미 받은 본인 DTO만 사용하고 raw replay를 새로 요청하지 않는다. 알 수 없는 상대·null 통계/시간·0표본·abort/cancel·첫 공식 완료와 이 기기 연습을 구분한다. 날짜/관측 시각/90일 범위를 표시하고 로컬 기록은 기존 데일리 화면으로 연결한다.

## 일반 조회의 인증·취소

MePort와 HTTP adapter는 Rust 원천의 Me DTO를 strict decode하고 응답의 계정·종류·필터·버전·기간/시각/목록 상한을 검사한다. cursor는 해석·저장하지 않는 opaque 문자열이며 POST 본문으로만 보낸다. JSON MIME/64KiB/redirect 거부/no-store와 현재 CSRF를 사용한다.

기존 AuthenticatedRequests의 같은 요청 집합/abort budget 안에 game/self scope를 둔다. 기본 game은 기존 닉네임 조건을 유지하고 self는 현재 계정·세션 revision만 요구한다. 두 scope 모두 전후 bootstrap과 auth generation/revision을 확인하며 self가 게임 권한을 발급하지 않는다. 새 독립 세션 저장소나 로그인 캐시는 만들지 않는다.

컨트롤러는 메모리에만 개인 DTO를 가진다. 필터/페이지/화면 변경은 이전 요청을 취소하고 새 요청이 시작될 때 기존 개인 목록을 지운다. 계정·세션 revision·오프라인·로그아웃·삭제·peer invalidation 변화는 즉시 데이터와 선택 상세를 지우고 늦은 응답을 무시한다. 동일 계정의 명시 변경 중에는 관리 폼을 유지하고 조회 DTO를 비우며, 변경 완료 뒤 현재 권한으로 다시 읽는다. 계정/실제 세션 상실 때는 폼과 확인 창도 제거한다. Apple 철회 재시도/수동 안내는 인증 서비스의 작업별 메모리 상태와 공통 피드백으로 유지해 계정 폼이 닫힌 뒤에도 사라지지 않게 한다. 실패를 빈 목록으로 바꾸지 않으며 로딩/빈 전적/오류/로그인 필요를 구분한다. 개인 화면의 robots meta는 noindex이고, 공개 shell/WASM 캐시는 유지하되 개인 API/스냅샷을 SW/localStorage/IDB에 넣지 않는다.

서버 현재 동의 view와 실제 브라우저 CMP 제어를 구분한다. 현재 선택/미확인/만료·확인/만료 시각·실CMP 미설정 상태를 표시한다. 저장 view는 선택 수집 권한이 아니며 새로운 수집을 활성화하지 않는다. 현재 동의의 만료 표시는 조회 시작 monotonic 시각과 서버 관측 시각부터의 잔여 시간을 사용해 응답/후속 proof 대기를 보수적으로 반영한다. 브라우저 시계나 이 표시는 서버 저장 권한을 발급하지 않는다. 기존 기기 ConsentSettings를 재사용해 비로그인 사용자도 계속 재열기·철회를 이용한다.

## 제거와 보존

파일 전용 UI·문구·JSON Blob/download sink·일괄 collector·AuthPort/transport export 메서드와 관련 DTO/생성 TS를 제거한다. 서버 privacy export 모듈, account export 전용 port/handler/projection과 이벤트 export projection을 제거한다. 세 구 공개 경로 `/api/v1/me/export`, `/api/v1/me/games/export`, `/api/v1/me/games/replay`는 미설정 route도 등록하지 않아404다.

데일리의 accepted receipt helper는 파일과 무관한 이름으로 바꾸고 Me/필수 이벤트의 검증에 재사용한다. 기존 migration·저장 데이터·replay 검증/보존 정리·필수 이벤트·계정 삭제·게임 결과 공유PNG·운영 백업은 유지한다. 기존 파일 테스트를 단순히 통과에서 삭제하는 것으로 끝내지 않고 같은 소유권/삭제/보존/공식 시간 경계를 조회 API와 실제 저장 자료에서 확인한다. 파일 전용 재생/일괄 수집 시나리오는 제거된 계약 이력으로 남긴다.

## 검증

문서→행동 Red→Green→Refactor. 일반 self scope의 닉네임 없음/오래된 인증, game scope 유지, 전후 소유권/취소·역순 응답·필터/페이지·오프라인·계정 전환/삭제·null/0표본·noindex/no-store를 검증한다. 실제 PG/HTTPS의 내 정보 조회·8언어·360px/PC·키보드·개인 파일 다운로드0·세 구 API404, 기존 대전/데일리/공유 회귀와 최신 coverage gate를 확인한다. 실제 CMP/OAuth 운영 연결·admin·전역403/404·#83 해결로 보고하지 않는다.
