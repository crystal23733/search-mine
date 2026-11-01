# ADR0080 · 인증 세션별 현재 동의 저장·철회 기반

- 상태: 채택 · 승인된 FR01/13/16·NFR01/02/05/07/08 · #174, 상위 #21
- 선행: #20/PR146·#156/PR157 병합·CLOSED
- 시나리오: TS20/21/24/29/31/35/36

## 문제와 결정

브라우저 ConsentPort의 valid view는 서버 저장 권한이 아니다. 실제 CMP 어댑터가 없는 현재 실행은 unavailable이며 선택 수집/광고를 차단한다. 서버도 기본 비활성 ConsentVerifier를 사용하며 HTTP·main에 연결하지 않는다. 테스트 어댑터를 제품에 주입하지 않는다.

인증된 계정의 현재 세션에 한해 서버 ConsentVerifier가 확인한 analytics/advertising 선택을 별도 PostgreSQL 저장소에 기록하는 기반을 둔다. verifier는 세션 ID와 서버 시각을 받아 같은 subject와 미래의 유효 기한을 반환한다. 클라이언트가 보낸 grant/revision/TC 문자열을 저장 권위로 받는 HTTP는 만들지 않는다. 실제 CMP/TCF 검증 방식은 선택한 서비스가 확정된 뒤 별도 어댑터에서 구현한다.

세션당 현재 행 하나만 저장한다. subject는 기존 auth_sessions.id, revision은 서버 UUID, checked_at/expiry와 두 목적 boolean만 둔다. 토큰·이메일·TC 문자열·추가 개인 식별자는 저장하지 않는다. 행은 세션 logout/회전/계정 삭제에 cascade하고 만료된 세션의 기존 cleanup을 따른다. 만료 후 조회는 기간 정보를 유지하되 두 목적을 허용하지 않는다. 별도 동의 이력/영구 식별자를 만들지 않는다.

검증 IO는 DB 잠금을 잡기 전에 수행한다. 결과를 저장할 때 account→session 순으로 잠그고 실제 token hash/account/session revision과 시각을 다시 확인한다. 검증 대기 중 철회·로그아웃·계정 삭제는 권한을 만들지 못한다. 전체 요청은2초이며 DB 대기·검증·commit 전의 시계 역행/세션 만료/증거 만료도 거절한다. 증거 유효 기간은 세션 expiry를 넘지 못한다.

refresh는 기대 revision과 현재 행의 revision을 비교한 뒤 새 서버 revision으로 바꾼다. 최초 None도 현재 행이 없어야 하므로 늦은 최초 grant가 앞선 철회를 덮지 못한다. withdraw는 현재 인증 세션의 선택한 목적만 false로 바꾸고 항상 새 revision을 발급한다. 만료/시계 역행은 grant로 복구하지 않는다. 다른 목적의 원래 유효 기한을 연장하지 않으며, 기존 행이 없을 때 철회는 두 목적 false인 행을 세션 기한까지 남겨 지연된 최초 grant를 막는다.

본인 내부 read는 같은 인증 transaction과 현재 session revision을 요구한다. 실제 계정 export HTTP와 선택 이벤트의 같은 저장 transaction 내 at-write 검증은 다음 필수 하위 작업이다. 그 전에는 main/HTTP에 store를 연결하거나 선택 이벤트 DB 제약을 풀지 않는다. 이 기반의 내부 read를 사용자 내보내기 완료로 표현하지 않는다. 무계정 로컬 연습은 서버 세션/행을 만들지 않으며 익명 CMP subject는 후속 설계다.

## 검증과 미완료 범위

Red/Green: 목적 분리·증거 subject/기간·기대 revision 충돌·기본 verifier 차단, 실제 PostgreSQL의 동시 grant/CAS·철회와 지연 grant·다른 세션/계정·logout/삭제 cascade·DB 잠금 중 역행/만료/2초를 검증한다. 새 도메인을 coverage95 검사에 포함하고 서버 전체80을 유지한다. 기존 앱의 선택 API 부재와 essential-only 제약도 유지한다.

#174는 실제 CMP/TCF·지역 검수·공개 동의 HTTP·본인 export HTTP·선택 이벤트 저장·광고·전체 정책·외부 #147을 완료하지 않는다. 상위 #21은 OPEN이다. 제품 업데이트 #83도 별도 OPEN이며 재시도/브라우저 설정 변경으로 해결을 가장하지 않는다.
