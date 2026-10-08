# ADR0022 · 메모리 세션과 OAuth 계정 화면

2026-10-08 · 채택 · #46 · FR01/10/11/16, NFR01/02/06 · TS01/19/26/31/34/35/36. 선행 #45의 병합/종료를 확인한 뒤 제품 구현한다. Google·Apple·Kakao·Naver를 필수로 표시하며 새로운 제공자는 공통 catalog/port를 확장한다.

## 세션과 브라우저 경계

AuthIdentity/Export 시각은 JSON 정수 초와 생성 TypeScript number로 일치시킨다. 서버 timestamp 범위0~253402300799는 JavaScript 안전 정수 이내다. Rust ts-rs의 기본 i64→bigint 선언은 실제 serde JSON(number)과 달라 해당 공개 필드의 ts(type="number")를 원천에 명시하며 수신 어댑터도 안전 정수를 검사한다.

AuthPort는 bootstrap·start·nickname·연결/재인증·내보내기·해제/탈퇴·로그아웃을 제공한다. 생성 Rust DTO를 사용하며 브라우저 HTTP 어댑터가 응답을 최소 allowlist로 검증한다. fetch는 same-origin credentials, cache:no-store, redirect:error와 deadline을 사용한다. API 오류는 안정된 code만 UI 상태로 옮기고 HTML/raw body/비밀값을 표시하지 않는다. authorize redirect만 선택한 제공자의 고정 HTTPS origin/path allowlist를 허용한다.

CSRF와 account/session_revision은 메모리에만 보관한다. HttpOnly session cookie를 JS로 읽거나 localStorage/IndexedDB/SW/공개 cache로 옮기지 않는다. 요청 generation으로 bootstrap/변경 응답의 역순 도착을 무효화하고401/연결 장애에서는 온라인 인증 권위를 철회한다. 내부 단조 revision을 PendingSubmissions의 숫자 revision으로 제공하며 서버 UUID revision·account·연결 상태 변경에 따라 갱신한다. 다른 탭에는 BroadcastChannel의 무정보 invalidation만 전송하고 visibility/online 때 서버 bootstrap을 다시 읽는다.

## 화면과 권리

브라우저 mutation 직전에 bootstrap으로 CSRF를 새로 받고 현재 account/session revision이 화면의 인증 권위와 같은지 확인한다. 계정 또는 세션이 달라지면 작업을 실행하지 않고 상태를 다시 확인하도록 안내한다. 파괴적 확인은 dialog를 열 때의 account UUID에 묶고 account 전환 후 확인으로 다른 계정을 삭제하지 않는다.

design_layout의02 로그인·02-1 nickname·13설정 card/큰버튼/고지와 토큰을 따른다. 제공자 비활성은 disabled와미설정 안내, 전체 장애는 재시도·무계정 연습을 제공한다. 첫 로그인 nickname은 프로필을 채우지 않고 게임용 이름만 입력하며 실제 Rust Unicode/NFC 검증 결과를 응답 권위로 사용한다. JS는 편의 문자수 안내만 계산하고 도메인 규칙을 복제하지 않는다.

연결 목록/해제·nickname 변경·export·logout·delete는 설정에서 분리한 AccountSettings organism으로 구현한다. 서버 reauth_required에는 연결된 제공자의 명시 재인증 UI를 표시하며 사용자는 돌아온 뒤 민감 작업을 다시 선택한다. 자동 삭제 재실행을 하지 않는다. 마지막 제공자 해제는 UI와 서버에서 보호한다. 해제/탈퇴 확인 dialog, 완료 후 전체 세션 종료/Apple 직접 철회 안내를 제공한다. 외부 철회가 확정됐다고 표현하지 않는다. 내보내기는 요청 후 메모리 JSON download로만 만든다.

onboarding/callback은 허용 return_path enum만 사용하고 외부 return URL을 따르지 않는다. callback 오류 query는 번역한 실패 안내만 표시한다. locale/초대 code는 기존 navigation 계약과 함께 보존하고 OAuth state/code를 client 저장소에 기록하지 않는다. nickname 미설정 계정도 설정에서 개인정보 권리를 행사할 수 있다. 온라인대전/공식 기록은 nickname 이후다. 정책/운영자 준비 #21을 완료했다고 주장하거나 현재 로그인 버튼에 가짜 약관 동의를 붙이지 않는다.

## 데일리와 검증

현재 account/session revision을 PendingSubmissions port에 연결한다. 공식 전송은 #19이며 이 단계에서는 unsupported 성공 응답으로 대기 항목을 지우지 않는다. 계정 전환/로그아웃/장애 중 응답은 원래 대기를 보존한다. 무계정 로컬 기록은 사용자 확인 전 자동 제출하지 않는다.

TDD로 최소 요청/잘못된 JSON·redirect/deadline/401·역순bootstrap/계정 전환/연결 장애·nickname 정규화 응답·민감 확인/재인증·pending 보존을 확인한다.8언어 동일 키/placeholder, keyboard/dialog·PC/mobile·scroll overflow·bundle 검사를 실행한다.

실제 HTTP E2E fixture는 Rust example에서 제품 account_auth_router/AuthService/PgAuthStore와 PostgreSQL18을 사용하고 제공자 proof만 테스트 port로 대체한다. 브라우저는 loopback HTTPS proxy에서 Secure/HttpOnly cookie를 실제 사용하며 제공자 navigation은 Playwright가 fixture code callback으로 돌려보낸다. 테스트 실행 파일/인증서/키는 public test-only이고 serving/main에 fixture 인증 경로를 추가하지 않는다. CI web job의 별도 DB에서 이 검증을 반드시 실행한다. 로컬 Docker 부재의 실DB E2E 미실행을 성공으로 계산하지 않는다. 실제 제공자·등록 audience/키는 #42 검수다.
