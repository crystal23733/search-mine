# ADR0088 · 브라우저 동의 관리와 현재 권한의 선택 전송

- 상태: 채택 · #190 / 상위21 · FR01/11/13/16·NFR01/02/05/06/07/08
- 선행: #182/186/188·PR189 병합/CLOSED, develop32e3cad
- 시나리오: TS19/20/21/22/24/26/29/31/35/36

## 동의 관리

사용자 잔여 작업 검토 뒤 진행 지시로 기존 [ADR0087](0087-optional-event-http.md)의 API를 웹에 연결한다. stateless ConsentControlPort는 read/refresh/withdraw, 인증 self scope·전후 proof·CSRF·same-origin/no-store/error redirect·bounded JSON을 사용한다. v1/계정/revision/현재/만료의 폐쇄 decoder와 기존 MeConsent view 검증을 공유한다. 클라이언트가 grant나 provider proof를 만들지 않는다.

내정보/privacy의 전용 controller가 현재 상태·원래 조회 시작 시각·working/error를 메모리에 보유한다. mount에서 조회하고 새로고침·목적별/전체 철회·CMP재검증은 명시적으로 실행한다. 신규 요청/계정 또는 auth revision 변경·실패·이탈은 기존 snapshot을 비우고 요청을 취소한다. 동일 계정의 nickname은 필요 없으며, 늦은 완료는 화면에 반영하지 않는다. 기존 MeConsent 표시는 snapshot의 observed_at/consent를 재사용하고 monotonic expiry를 유지한다. 현재 CMP가 없으면 재검증을 비활성화하지만 조회/철회는 제공한다.8언어 공통 버튼·취소/오류/다시시도와 focus를 검증한다. 민감한 파일 다운로드는 복원하지 않는다.

## 전송과 원천

OptionalEventCollector는 DI된 AuthPort·ConsentPort·ConsentControlPort·EventTransport를 사용한다. 현재 브라우저의 valid purpose와 연결된 인증 self owner가 있어야 시작한다. 보내기 직전에 서버 read로 현재 revision/목적/기한을 확인하고, expected_revision을 담아 단건 저장한다. 서버의 같은 transaction 검증이 최종 권위다. 캐시한 snapshot·브라우저 선택만으로 수집을 허용하지 않는다. 실제 CMP 없는 main은 UUID/탭키·선택API요청을 만들지 않는다.

기존 Rust의 event 목적 도출을 protocol의 OptionalEventPurpose/method로 이동하고 서버 domain은 이를 변환한다. 동일 메서드에서 생성하는 enum tag→purpose map을 TS로 내보내며 손으로 목적 규칙을 중복하지 않는다. 입력/receipt type은 기존 Rust wire를 그대로 사용한다. 날짜/게임/동의 규칙을 TS에 새로 구현하지 않고, client는 자신의 요청과 현재권한에 대한 응답 검증만 한다.

각 기록의 ID/occurred_at은 한 번 만들고 탭 key는 계정/auth revision 수명 내 메모리에만 보유한다. 동시에1건·기록 호출부터 전체2초, pending이면 즉시 drop하고 queue·자동retry·sendBeacon/keepalive·영구저장소를 사용하지 않는다. 최소 receipt의 ID/시각을 확인하고 요청 취소/권한 변경/만료 뒤 늦은 ack는 성공으로 보고하지 않는다. 브라우저/서버 동의 목적은 모두 검사한다. 시계 역행과 monotonic 기한을 확인하고 서버 snapshot 잔여 기한은 read 시작 시각 기준으로 보수적으로 적용한다.

동의 mutation 시작 전에 collector를 invalidate한다. auth/브라우저 동의 변경·화면 이동·숨김·pagehide·오프라인·복원은 진행 요청과 탭key를 취소/삭제한다. listener/timer는 dispose에서 정리한다. 이 단계의 실제 UI producer는 공개 landing route(home/daily/rules/tutorial)뿐이며 라우트가 아닌 query/URL·개인정보를 보내지 않는다. 전송 Promise는 게임/탐색에서 기다리지 않고 실패를 UI 오류로 올리지 않는다. 그 밖의 tutorial/rematch/share/광고 hook은 해당 producer 작업에서 연결한다.

## 검증과 한계

시나리오→실제 행동Red→Green 뒤 current self proof/strict decoder·동의 갱신/CAS/철회·이탈/late 완료·브라우저와 서버 grant 교집합/만료/계정/취소/원래2초·최대1·메모리 재시도없음·Rust생성 목적 일치를 검증한다. 실HTTPS에서 nickname 없는 조회/철회·8언어·화면 이동/로그아웃 취소·CMP미설정 선택요청0과 게임 계속을 확인한다. 실제 서버grant는 Rust PG 검증의 시험 verifier로만 만들며 실행 fixture에 거짓grant endpoint를 추가하지 않는다.

실제CMP/TCF/지역검증·운영147/42·익명subject·SDK/광고·전체정책·상위21/83은 후속이다. 외부키 없이 기반을 검증하되 실제 CMP 성공이나 수집운영을 완료로 보고하지 않는다. 기존 retry/브라우저 flags·제품 게임규칙·DB migration/의존성을 바꾸지 않는다.
