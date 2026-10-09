# ADR0031: OAuth 친구 초대의 서버 거래 보존

2026-10-09 · 승인된 #17/FR01/07/08/11의 구현 구체화 · #64 병합 뒤 순차 진행. 외부 OAuth 키/실계정 #42는 별도 출구다.

## 결정

친구 초대는 로그인 start의 선택 `invite_code`로 전달한다. 목적지는 기존 닫힌 ReturnPath이고 `friends`+login에서만8문자 canonical RoomCode를 허용한다. RoomCode는 공유 서버 값 타입으로 옮기고 로비와 인증이 같은 검증을 사용한다. 인증이 로비 policy/service를 참조하지 않으며 TS에 게임 규칙을 복제하지 않는다.

ReturnPath의 FriendsInvite(RoomCode)가 코드와 목적지를 결합한다. AuthService의 기존 start/start_localized API는 유지하고 account link/reauth에는 초대를 거절한다. 임의 URL·fragment·경로/쿼리 주입·다른 목적지의 코드를 허용하지 않는다. 로그인 JSON은 unknown/duplicate 필드를 거절한다.

nullable `auth_transactions.invite_code`를 새 migration으로 추가한다. DB는 정확8byte/허용 문자·login/friends의 결합을 CHECK하고 기존 거래의 NULL은 호환된다. insert와 matching5분 원자 DELETE RETURNING이 코드를 함께 저장·복원한다. 계정·세션·공식 기록·이벤트에 복사하지 않고 만료 청소는 기존 transaction 수명을 따른다. 공급자 authorize URL/nonce/PKCE에는 초대를 넣지 않는다.

검증된 각 transaction의 locale/destination/code만 callback의 로컬 redirect에 사용한다. 성공은 friends?code 또는 onboarding?return_path=friends&code로 복원한다. provider 취소/교환 실패도 거래를 소비하고 login?error=auth_failed&return_path=friends&code로 복귀한다. 소비할 수 없는 state·다른 browser·expiry·재전송은 기존 안정된 실패로 거절하고 임의 입력에서 초대를 복원하지 않는다. 중첩/역순 콜백은 각 transaction의 목적지를 따른다.

브라우저 AuthPort/Transport의 login만 선택 코드를 전달한다. nickname skip/tutorial와 locale 변경은 현재 URL의 code를 보존한다. 이 코드는 게임용 초대이며 OAuth code/state/token과 다르다. 추가 localStorage/sessionStorage/IndexedDB를 만들지 않는다. 앱 문서의 no-referrer 정책으로 외부 이동 시 초대 URL을 보내지 않는다. callback/auth 응답의 no-store/no-referrer는 유지한다.

## 검증과 경계

서버 HTTP의 invalid code/목적지·duplicate·canonical·별도 거래/역순·성공/취소/교환 오류·nickname 보유 여부와 기존 브라우저 바인딩/만료/일회 소비를 행동 시험으로 확인한다. 실제 DB의 nullable CHECK·insert/원자 consume·다른 browser와 exact expiry를 실행하고 HTTPS 브라우저는 네 제공자/8locale·nickname/tutorial skip·저장소 거부/취소 재시도·secret 미보관을 검사한다. 실제 provider proof만 port로 대체하며 키 없는 실계정 완료를 주장하지 않는다.

이 작업은 초대 복귀까지다. 실제 로비·온라인 보드/게임 화면과 재접속의 통합 출구는 다음 #17/#18 하위 작업이며 서버 board/seed를 브라우저에 보내지 않는다. 초대의 존재/만료/seat/열거 제한은 인증 성공 뒤 기존 로비 HTTP의 실제 상태가 결정한다.
