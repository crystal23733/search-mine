# 19. 테스트 시나리오

TS18/20/26/30 확장(#159): 앱 없는 두 탭의 첫 제어권 준비 후 동일workerURL bytes를 갱신한다. control과 stopping 직후 재시작 요청/실제old재시작/원래10초reload를 구분한다. Browser trace의 다른 소유자 보호·동기/누락 완료·상한/중복 정리를 확인하고 자연 E2E의 최초 실패·native 자료를 CI에 보존한다. [ADR0075](../adr/0075-native-worker-restart-observation.md). #83 제품 수정 완료를 주장하지 않는다.

TS21/27 확장(#141): 공개 tail seed5108을 실제 생성·인증하고 기존 main 캐시 준비8초 안에 완료한다. dev/test 빌드에서 코어만 opt1이며 debug assertion/overflow·RNG/규칙/실제 OsSeedSource를 유지한다. 실제 PG·native main 재시작/취소·계측을 확인하고 단위 준비 예산과 미니 PC 목표를 구분한다. [ADR0074](../adr/0074-native-preparation-build-profile.md).

> 적용: pm-execution:test-scenarios · 입력: [Stories](17-stories.md), [PRD](16-PRD.md)
> 설계 단계 시나리오이며 테스트 코드를 실행했다는 뜻이 아니다. 각 항목은 목적·전제·역할·행동·예상 결과·경계를 함께 정의한다.

## TS01: OAuth 시작·닉네임·삭제 (US01)

- 목적: 기존 계정으로 최소 정보만 제공해 서비스 계정을 만들고 삭제한다.
- 전제·역할: fake provider/Clock/Repository, 최초 방문자.
- 단계와 각 결과: 4개 제공자 중 하나로 인증 → 검증된 식별자만 매핑; 닉네임 → 계정 완성/세션 회전; 삭제 → 세션 철회·신원/순위 식별자 제거; 이전 쿠키 → 거절.
- 최종 기대·예외: 비밀번호 입력/API/컬럼 없음; 이메일/실명/사진 미보관; 1/2/16/17 grapheme·NFC 경계; 취소 시 원래 화면 복귀.
- 테스트 경계·계층: AuthService / OAuthProvider / AccountRepository / 통합+E2E.

## TS02: 동일 판·비밀 미전송 (US02)

- 목적: 동일 판·비밀 미전송의 관찰 가능한 행동을 보장한다.
- 전제·역할: 고정 seed/버전/오프닝, 두 플레이어.
- 단계와 각 결과: native와 WASM local fixture를 생성한다 → 같은 판; online snapshot을 받는다 → 공개 필드만.
- 최종 기대·예외: 지뢰/seed/상대 셀/lie truth가 JSON·DOM·cache·로그에 없음.
- 테스트 경계·계층: BoardGenerator / SnapshotProjection / 속성+통합.

## TS03: 노게스 증명 (US02)

- 목적: 노게스 증명의 관찰 가능한 행동을 보장한다.
- 전제·역할: 작은 판 전수 집합·256셀 seed corpus, solver.
- 단계와 각 결과: 안전 추론을 반복한다 → 증명된 칸만; 완료한다 → safeTotal 일치; 막힌 판을 생성기에 준다 → 거절.
- 최종 기대·예외: solver가 정답을 읽지 않고 논리 증거를 내며 timeout은 채택 안 됨.
- 테스트 경계·계층: KnowledgeSolver / BoardGenerator / 단위+proptest.

## TS04: 열기·flood·flag (US03)

- 목적: 열기·flood·flag의 관찰 가능한 행동을 보장한다.
- 전제·역할: zero 영역·flag·mine fixture, 플레이어.
- 단계와 각 결과: flood를 연다 → 새 안전칸 증가; 같은 cell 재전송 → 증가 없음; flag 셀 open → 거절.
- 최종 기대·예외: 초기 opening gauge0, 후속 새 셀당1, mine은 진행률0.
- 테스트 경계·계층: RuleEngine / PlayerBoard / 단위.

## TS05: 기절 경계 (US03)

- 목적: 기절 경계의 관찰 가능한 행동을 보장한다.
- 전제·역할: fake tick=0, mine 셀, 플레이어.
- 단계와 각 결과: mine을 연다 → stun3000; tick2999 입력 → 거절; tick3000 입력 → 허용.
- 최종 기대·예외: 중첩 stun은 max deadline, 연결 동기화는 허용.
- 테스트 경계·계층: RuleEngine / Clock / 단위.

## TS06: 공격 후보·게이지 (US04)

- 목적: 공격 후보·게이지의 관찰 가능한 행동을 보장한다.
- 전제·역할: gauge20, truth1/8/0·공개 셀·기존 overlay fixture.
- 단계와 각 결과: attack한다 → 닫힌 비제로±1만; 최대2나 후보없음 → 실패; 공개 상태를 조사한다 → 위치 숨김.
- 최종 기대·예외: 1→2/8→7, 실패는 gauge 보존, 성공0, 셀 중복 없음.
- 테스트 경계·계층: LieValidator / RuleEngine / 단위+속성.

## TS07: 간파 증명·두 lie (US04)

- 목적: 간파 증명·두 lie의 관찰 가능한 행동을 보장한다.
- 전제·역할: 관측은 같지만 진실 다른 작은 판 모델, 검증자.
- 단계와 각 결과: 모든 모델의 안전 행동을 찾는다 → 증거; 특정 모델만 가능한 경로 → 거절; 두 lie 조합 → 함께 검증.
- 최종 기대·예외: 한 경로 정답 참조·모호한 lie 식별·budget 초과는 적용 불가.
- 테스트 경계·계층: LieValidator / KnowledgeSolver / 전수+proptest.

## TS08: 정확한 지목·반사 (US05)

- 목적: 정확한 지목·반사의 관찰 가능한 행동을 보장한다.
- 전제·역할: 공개 lie, attacker gauge>0, fake clock.
- 단계와 각 결과: defender가 accuse한다 → truth 복원; attacker 상태 확인 → 2초 stun·gauge0; command 재전송 → 1회 효과.
- 최종 기대·예외: 본인 성공 지목은 기절 없음, active overlay 제거.
- 테스트 경계·계층: RuleEngine / Clock / 단위.

## TS09: 오지목·입력 모드 (US05)

- 목적: 오지목·입력 모드의 관찰 가능한 행동을 보장한다.
- 전제·역할: 참 숫자·닫힌 셀·0 fixture, 키보드/터치 사용자.
- 단계와 각 결과: 참 숫자를 지목 → 본인3초; 닫힌/0 지목 → invalid; long press 후 click → 1명령.
- 최종 기대·예외: 우클릭 메뉴·A 모드·touch 모두 같은 intent.
- 테스트 경계·계층: RuleEngine / InputController / 단위+E2E.

## TS10: 10초 봇 백필 레이스 (US06)

- 목적: 10초 봇 백필 레이스의 관찰 가능한 행동을 보장한다.
- 전제·역할: fake clock 9999/10000, 대기자2·매칭자.
- 단계와 각 결과: 기한 전 human 예약 → human 한 번; 기한의 예약 → bot 또는 다음 큐; cancel → 시작 없음.
- 최종 기대·예외: 한 queue에 match_id 하나, 봇 표시와 난이도3.
- 테스트 경계·계층: Matchmaker / Clock / 통합.

## TS11: 봇 정보 제한 (US06)

- 목적: 봇 정보 제한의 관찰 가능한 행동을 보장한다.
- 전제·역할: 같은 PublicView·다른 hidden Board fixture, BotPolicy.
- 단계와 각 결과: 같은 view로 choose → 동일 의도; 난이도 변경 → 간격 변화; 막힌 판 → 확정 지목/대기.
- 최종 기대·예외: 봇에 Board 참조 없음, 숨은 정답 차이가 입력에 영향 없음.
- 테스트 경계·계층: BotPolicy / CorePort / 단위+속성.

## TS12: 친구 방 (US07)

- 목적: 친구 방의 관찰 가능한 행동을 보장한다.
- 전제·역할: host/friend/third account, 만료 clock.
- 단계와 각 결과: create→code/link; friend join→2seat; third join→full; 만료 후 join→expired.
- 최종 기대·예외: room code가 player session을 대체하지 않음, 중복 탭·ready 경계.
- 테스트 경계·계층: RoomService / AccountRepository / 통합+E2E.

## TS13: 튜토리얼 (US08)

- 목적: 튜토리얼의 관찰 가능한 행동을 보장한다.
- 전제·역할: 첫 방문·저장소 사용 불가 fixture, 초보자.
- 단계와 각 결과: 접속→tutorial; attack1회→안내; accuse1회→반사; skip/replay→홈/재학습.
- 최종 기대·예외: 30초 이해는 수동 E1 관찰, 초대 링크 유지, 광고0.
- 테스트 경계·계층: TutorialController / StoragePort / E2E+수동.

## TS14: 종료·무승부·동시 입력 (US03)

- 목적: 종료·무승부·동시 입력의 관찰 가능한 행동을 보장한다.
- 전제·역할: same ingress tick, seat0/1, 마지막 safe 셀, deadline.
- 단계와 각 결과: 입력을 직렬 적용 → 먼저 commit 완료 승리; 종료 후 attack→거절; deadline 동률진행→draw.
- 최종 기대·예외: 승패1회, timeout 이후 입력 무효, 규정 순서 fixture 재현.
- 테스트 경계·계층: MatchActor / RuleEngine / Clock / 단위+통합.

## TS15: 재접속·중복 (US14)

- 목적: 재접속·중복의 관찰 가능한 행동을 보장한다.
- 전제·역할: revision gap, 끊긴 player, fake clock.
- 단계와 각 결과: resume→권위 snapshot; old command retry→cached ack; grace30초 초과→forfeit.
- 최종 기대·예외: server time 계속 흐름, old epoch 거절, flag/gauge 중복 없음.
- 테스트 경계·계층: MatchActor / Transport / Clock / WS통합+E2E.

## TS16: 서버 장애·두 명 끊김 (US14)

- 목적: 서버 장애·두 명 끊김의 관찰 가능한 행동을 보장한다.
- 전제·역할: 진행중 매치·pending 저장, 운영자.
- 단계와 각 결과: 둘 모두 grace 만료→abandon draw; 서버 재시작→abort; 복구→결과 저장1회.
- 최종 기대·예외: abort 승패/광고 카운트 제외, 로컬 권위 전환 금지.
- 테스트 경계·계층: MatchRecovery / MatchRepository / 통합.

## TS17: UTC 데일리·순위 (US09)

- 목적: UTC 데일리·순위의 관찰 가능한 행동을 보장한다.
- 전제·역할: UTC 전후 날짜·공개 seed version, 제출자.
- 단계와 각 결과: 동일 date 로드→동일 판; 유효 replay 제출→첫 완료; duplicate attempt→같은 결과.
- 최종 기대·예외: 동시 제출 unique, modified log 거절, online/offline timing 구분.
- 테스트 경계·계층: DailyService / ReplayVerifier / 통합+E2E.

## TS18: 오프라인 캐시·제출 (US10)

- 목적: 오프라인 캐시·제출의 관찰 가능한 행동을 보장한다.
- 전제·역할: 캐시 있음/없음·IndexedDB 실패, 솔로.
- 단계와 각 결과: network off→local 표시; daily complete→대기저장; 복구→제출; 만료 verifier→개인 기록.
- 최종 기대·예외: 미캐시 최초 접속 무보장, 중복 제출 없음, 비밀·광고 캐시 없음.
- 테스트 경계·계층: CorePort / StoragePort / NetworkPort / E2E.

## TS19: 8언어·URL·폴백 (US11)

- 목적: 8언어·URL·폴백의 관찰 가능한 행동을 보장한다.
- 전제·역할: 8locale·누락 키·browser ja-JP/zh-Hant, 방문자.
- 단계와 각 결과: localized URL→우선; 언어변경→URL/설정 갱신; 누락→en; unsupported→en.
- 최종 기대·예외: match 유지, canonical/hreflang/x-default, placeholder/복수형 일치.
- 테스트 경계·계층: I18nPort / Router / 단위+E2E.

## TS20: 인증·CSRF·Origin (US01)

- 목적: 인증·CSRF·Origin의 관찰 가능한 행동을 보장한다.
- 전제·역할: 외부 origin·expired cookie·old epoch·XSS nickname, 공격자.
- 단계와 각 결과: WS handshake/HTTP write→거절; nickname 렌더→문자 escape; DB 문자열→바인딩.
- 최종 기대·예외: 세션 원문 미로그, DML 권한으로 schema 변경 불가.
- 테스트 경계·계층: AuthAdapter / SqlxRepository / 통합.

## TS21: 입력 폭주·검증 (US04)

- 목적: 입력 폭주·검증의 관찰 가능한 행동을 보장한다.
- 전제·역할: 8KiB 초과·negative cell·unknown v·20/s 초과, 공격자.
- 단계와 각 결과: 각 invalid frame 전송→안정 error/close; 생성폭주→bounded reject.
- 최종 기대·예외: CPU/메모리 무제한 증가 없음, 다른 매치 진행 유지.
- 테스트 경계·계층: ProtocolDecoder / RateLimiter / 통합+k6.

## TS22: 모든 공개 경로의 비밀 (US04)

- 목적: 모든 공개 경로의 비밀의 관찰 가능한 행동을 보장한다.
- 전제·역할: active lie·상대 공개 정보·secret seed fixture, 사용자.
- 단계와 각 결과: snapshot/delta/error/aria/cache/log 수집→공개 DTO만.
- 최종 기대·예외: lie boolean·truth·source·상대 cells·online seed 없음.
- 테스트 경계·계층: SnapshotProjection / BoardRenderer / 통합+E2E.

## TS23: 광고 3판 cap (US12)

- 목적: 광고 3판 cap의 관찰 가능한 행동을 보장한다.
- 전제·역할: fake AdsPort·completed_count0, 사용자.
- 단계와 각 결과: 1/2판→요청0; 3판→최대1; 4/5→0; 6→최대1; no-fill→slot 소비.
- 최종 기대·예외: 다중 탭·반복 result 진입·callback 중복에서 이중 요청 없음.
- 테스트 경계·계층: AdsPolicy / StoragePort / 단위+E2E.

## TS24: CMP 거부·실패·철회 (US13)

- 목적: CMP 거부·실패·철회의 관찰 가능한 행동을 보장한다.
- 전제·역할: fake CMP valid/denied/unknown/error, 사용자.
- 단계와 각 결과: 거부→게임 시작; 실패→광고0; 동의→safe screen SDK1회; 철회→추가요청0.
- 최종 기대·예외: 분석 purpose 별도, 정책/저장소 고지·재열기, 실제 인증 CMP 별도 수동 검수.
- 테스트 경계·계층: ConsentPort / AdsPort / E2E+수동.

## TS25: 플레이 중 광고 금지 (US12)

- 목적: 플레이 중 광고 금지의 관찰 가능한 행동을 보장한다.
- 전제·역할: countdown/playing/tutorial/offline, 늦은 SDK callback.
- 단계와 각 결과: 각 상태로 이동→slot 제거/요청0; callback 늦게 도착→현재 상태 재검사.
- 최종 기대·예외: no-fill/timeout/차단 뒤 다음판 무차단, 배너 오클릭·초점 확인.
- 테스트 경계·계층: AdsPolicy / MatchController / 단위+E2E.

## TS26: 접근성·작은 화면 (US11)

- 목적: 접근성·작은 화면의 관찰 가능한 행동을 보장한다.
- 전제·역할: 360px·키보드·screen reader·reduced-motion, 사용자.
- 단계와 각 결과: 모드 변경→열기/깃발/지목; zoom/pan→tap 중복없음; 결과→focus 복구.
- 최종 기대·예외: 44px actions, 공개 label만, 색 이외 구분, 긴 번역 overflow 없음.
- 테스트 경계·계층: InputController / DOM Grid / E2E+수동.

## TS27: 성능·설정 snapshot (US15)

- 목적: 성능·설정 snapshot의 관찰 가능한 행동을 보장한다.
- 전제·역할: 미니 PC 사양·build commit·10k seed·k6, 운영자.
- 단계와 각 결과: 설정수정→기존match 불변; newmatch→새hash; 부하증가→admission cap.
- 최종 기대·예외: 생성p95≤100ms/입력p95≤20ms 목표와 실패율·global RTT 별도 기록.
- 테스트 경계·계층: RulesLoader / BenchHarness / 벤치+k6.

## TS28: 백업·복구·migration (US16)

- 목적: 백업·복구·migration의 관찰 가능한 행동을 보장한다.
- 전제·역할: 별도 빈 DB·암호화 dump·이전 앱, 운영자.
- 단계와 각 결과: pg_restore→무결성; migration→이전앱 호환; image rollback→대표 API 성공.
- 최종 기대·예외: RPO24h/RTO2h 목표 실측, checksum·외부매체, DB host포트0.
- 테스트 경계·계층: BackupRunbook / SqlxRepository / 통합+운영훈련.

## TS29: 이벤트·삭제·보존 (US16)

- 목적: 이벤트·삭제·보존의 관찰 가능한 행동을 보장한다.
- 전제·역할: 중복 event_id·30일/180일 데이터·delete account, 운영자.
- 단계와 각 결과: 중복저장→한 건; retention 실행→삭제/집계; account삭제→토큰철회·식별자제거.
- 최종 기대·예외: 정답·IP·토큰 미포함, 비필수 동의거부면 client 분석0, 백업잔존 고지.
- 테스트 경계·계층: EventRepository / RetentionService / 통합.

## TS30: 타입·코어 경계·TDD CI (US15)

- 목적: 타입·코어 경계·TDD CI의 관찰 가능한 행동을 보장한다.
- 전제·역할: public schema 변경·검사 pipeline, 기여자.
- 단계와 각 결과: DTO 변경→TS생성 diff; unsafe internal DTO→검사실패; Red→Green 증거 PR 첨부.
- 최종 기대·예외: 도메인 infra 의존0, coverage95/80 목표, 승인전 제품파일 CI거절.
- 테스트 경계·계층: TypeGenerator / CI / 정적검사+CI.

## TS31: 최소 권한·정보 allowlist (US01)

- 목적: Google·Apple·카카오·네이버가 필요한 정보만 요청/보관한다.
- 전제·역할: 어댑터별 authorize URL, 콘솔 권한 체크리스트, 불필요한 claim이 포함된 토큰/profile fixture.
- 단계와 각 결과: Google/Kakao → openid만; Apple → name/email scope 없음; Naver → 기본 id만; claim 매핑 → identity digest/내부 id/닉네임만; 공개 API/DB/로그/event 검사 → 이메일·실명·사진·전화·비밀번호 없음.
- 예외: Apple 철회용 refresh token은 CredentialVault에서 암호화 보관, 공개 응답/로그/내보내기에 없음. 콘솔 설정과 4개 실제 계정 검수도 수행한다.
- 경계·계층: OAuthProvider / IdentityMapper / CredentialVault / 계약+실DB+수동.

## TS32: OAuth 트랜잭션 바인딩·재전송 (US01)

- 목적: 로그인 CSRF·callback replay·code 탈취를 거절한다.
- 전제·역할: 브라우저 A/B, provider/intent 다른 state, 5분 경계, 동시 callback.
- 단계와 각 결과: 누락/위조/다른 브라우저 state → 거절; 만료/재사용/다른 provider나 link intent → 거절; 동시 정상 callback → 1회만 소비; 허용 없는 return URL → 거절; Google verifier 불일치 → 거절.
- 예외: 오류/취소도 거래를 소비, callback token/code/body 로그 없음; form_post 도입 시 전용 cookie 경계 검증.
- 경계·계층: AuthTransactionRepository / AuthService / 통합+E2E.

## TS33: 제공자 신원 검증·공급자 추가 (US01)

- 목적: 검증되지 않은 token/profile과 공급자 혼동을 막는다.
- 전제·역할: JWKS rotation, 틀린 sig/alg/iss/aud/azp/nonce/exp/sub, Naver 실패/빈 id, 비활성 provider.
- 단계와 각 결과: 각 변조 fixture → 거절; 검증된 응답 → 최소 identity; token endpoint 실패 → 계정/세션 생성 없음; 알 수 없는 provider → allowlist 거절; fake 새 adapter 등록 → AuthService 변경 없이 계약 재사용.
- 예외: OIDC 아닌 Naver token을 JWT로 신뢰하지 않음; 오류는 안정 코드, raw 응답 미노출.
- 경계·계층: OAuthProvider / ProviderRegistry / 계약+통합.

## TS34: 계정 연결·해제 (US01)

- 목적: 같은 이메일/닉네임으로 계정이 자동 합쳐지거나 탈취되지 않는다.
- 전제·역할: 계정 A/B, 같은 이메일 fixture, 다른 subject, 최근 재인증 세션.
- 단계와 각 결과: 독립 제공자 로그인 → 별도 계정; A가 재인증 후 새 provider 연결 → A에 추가; B에 연결된 subject → conflict; 마지막 수단 해제 → 거절; 두 callback 레이스 → DB unique 보호.
- 예외: link 시작 시 계정과 intent 고정, 중도 세션 변경은 거절, 성공 후 세션 회전.
- 경계·계층: AuthService / AuthIdentityRepository / 실DB+E2E.

## TS35: 내보내기·계정 삭제·토큰 철회 (US01/16)

- 목적: 추가 개인정보 없이 본인 데이터를 내보내고 목적 종료 시 삭제한다.
- 전제·역할: 여러 세션/제공자/순위/이벤트/백업, revoke 실패 fixture.
- 단계와 각 결과: 재인증 export → 본인 최소 데이터, credential/subject/숨은 판 제외; delete → 전 세션·identity·닉네임/순위 연결 제거; Apple revoke → credential 제거; 공급자 장애 → 로컬 삭제 완료, 제한 retry/수동 안내; 백업 restore → 삭제 tombstone 재적용 후 ready.
- 예외: 철회 queue 최대24h·백업 최대28일 고지, 서명 없는 Apple 알림 거절, 삭제 완료 여부를 정확히 표시.
- 경계·계층: AccountService / CredentialVault / RetentionService / 실DB+E2E+복구훈련.

## TS36: 로그인 없는 로컬 연습·화면 기준 (US01/08/10/11/13)

- 목적: 인증/광고 동의 부담 없이 로컬 연습과 접근성 설정을 제공한다.
- 전제·역할: 비로그인·광고 거부·provider 장애·캐시 유무, 360/768/1440px 화면.
- 단계와 각 결과: 로컬 연습 → 서버 계정/identity 생성 없음; 온라인/공식 제출 → 로그인 안내; OAuth 취소 → 링크/언어 유지; 광고 거부 → 게임 가능; 18개 참조 화면 상태 → 보안/규칙 명세와 일치.
- 예외: 무계정 결과 자동 업로드 없음, 로그인 후 명시적 제출만; 첫 오프라인 미캐시 안내; 4개 필수 제공자 노출, 선택 분석/광고 별도.
- 경계·계층: AuthPort / Router / StoragePort / UI E2E+시각·키보드 수동.

## 결정 사항

TS01~36을 백로그 이슈와 PR 검증 근거에 연결한다. fake는 도메인 경계에 주입하되 DB·WS 통합은 실제 구현체로 확인한다. 단위·속성·E2E·수동·성능을 서로 대체하지 않는다.

## 열린 질문

실제 저사양 모바일 모델, 미니 PC 사양, 외국어 검수자와 CMP 테스트 계정이 필요하다. 준비되지 않은 환경의 테스트는 통과로 기록하지 않는다.

TS16 확장(#93): [ADR0042](../adr/0042-unknown-result-details.md)의 unknown abort는 elapsed/own 모두 null·server_failure/abort/false다. 부분 null/타 reason/outcome/true·오염은 fail-closed. 이전 migration known행 보존·16개 통계 null패턴·writer 충돌·삭제 비복원·본인200/타인404·8언어 숫자 미생성·actual PG/HTTPS PC/mobile를 검사한다. SQL fixture는 계약 검증이며 실제 프로세스 재시작의 증거가 아니다.


TS16 확장(#107): [ADR0049](../adr/0049-reconnect-integration-exit.md)의 두 실제 WS upstream EOF 이후에 clock을 진행하고 abandoned/draw/completed:true·known own/elapsed·active0/final1/anchor를 검증한다. 쿠키만 새 context의 ID없는 최신 조회·8언어/모바일·추가 clock/OS restart 불변과 상위18 출구를 [검증107](../verification/107-reconnect-integration-exit.md)에 연결한다.


TS17/18의 #109 foundation은 [ADR0050](../adr/0050-bounded-native-daily-replay.md)에 따라 server published snapshot과 기존 native/WASM golden을 같은 Rust로 재생한다. metadata/identity/time 불일치·미완료는 거절하고 정상 rejected 클릭/정확한 중복은 core 계약을 유지한다. caller timeout/cancel 뒤 actual worker permit을 유지하며 첫 유효 완료/공식 시간/DB/웹 제출은 후속19에서 통합 검증한다. [실행109](../verification/109-bounded-native-daily-replay.md).


#112 FR09/10·NFR01/02/05/07/08·TS17/18/20/21/31/35/36 → [ADR0052](../adr/0052-daily-first-completion-store.md) → immutable accepted publication/worker payload binding·실제PG 첫 완료/attempt·권한/삭제·7일/90일/동시성 → [검증112](../verification/112-daily-first-completion-store.md). HTTP/timing/순위/rights export/UI는 다음 필수 하위다.


#114 FR01/09/10/16·NFR01/02/05/07·TS17/18/20/29/31/34/35/36 → [ADR0053](../adr/0053-personal-game-data-export.md) → 본인 game export bounded pages/raw·기존 auth transaction/fresh proof·삭제/만료/CPU 수명·명시 다운로드 → [검증114](../verification/114-personal-game-data-export.md). 공식 데일리 수집/timing/순위는 다음 하위다.


TS35 확장(#114): 본인에 속한51개 이상 online/daily 기록과 타인/만료/손상 기록을 실제PG에 준비한다. nickname 없는 최근 인증도 페이지/내보내기를 허용하며 다른 account/session revision·300초 경계·동시 삭제/철회는 거절한다. record UUID tie/커서 진행·raw exact7일/NULL/metadata-digest 결합과 record90일을 검사한다. HTTP가 데이터를 얻은 뒤 실제 session이 바뀌거나 barrier가 철회되면 응답을 commit하지 않는다. 실제 HTTPS 설정 버튼으로 PC/mobile·8언어에서 파일의 본인 기록/비밀 제외를 검사하고, 중간 page/raw 실패·반복 cursor·상한 초과·응답 직전 계정 전환은 파일이 생기지 않는지 확인한다. 정상 빈 목록과 upstream/DB 실패를 구분한다.


#116 FR01/09/10/16·NFR01/02/05/07/08·TS17/18/20/21/31/35/36 → [ADR0054](../adr/0054-authenticated-daily-submission-http.md) → closed 인증 Daily definition/submit/own·accepted snapshot/공유 bounded CPU·첫 완료/Practice·현재 세션 응답 장벽 → [검증116](../verification/116-authenticated-daily-http.md). trust는 verified_completion이며 ticket/heartbeat/순위/명시 UI는 후속19다.

TS17/18 확장(#116): 고정 POST와 실제 현재 세션/닉네임·CSRF로 accepted definition→공유 replay→첫 완료/같은 attempt/다른 attempt Practice→own/rights export를 연결한다. malformed/추가 필드/2MiB/버전/metadata·타인/revision/철회/삭제·CPU/SQL/기한/rate·최종 proof와 clock 역행을 검사한다. 원래 attempt와 receipt attempt를 구분하고 client elapsed를 verified_time으로 사용하지 않는다. 정상 own:null과 실패를 구분하며 실제PG/HTTPS로 같은 production 구성을 실행한다.


#118 FR09/10·NFR01/02/05/07/08·TS17/18/20/21/31/35/36 → [ADR0055](../adr/0055-bounded-daily-time-authority.md) → pure time policy/bounded authority·actual native evidence/45초/TTL/immutable retry·철회/재시작 → [검증118](../verification/118-daily-time-authority.md). 이 단계는 공개 수집·DB·HTTP·UI를 활성화하지 않으며 후속 rights export 확장 뒤 순차 연결한다.

TS17/18 확장(#118): 단조 fake Clock과 actual native golden 완료를 사용해 ticket 시작/동일 재시도/교체·계정당1개/전체capacity·heartbeat15초/45초 정확 경계·중복 ack가 시각을 연장하지 않음·고정 countdown/duration TTL·elapsed 최소와 immutable finish digest를 확인한다. 동시 finish, revoked/다른 registry lease, 실제 invalidation barrier 동안 late bind, clock 역행/overflow·새 registry no-proof를 검증한다. 공개 시간이 수집되지 않는 foundation과 이후 실PG/HTTP 시간 기능을 구분한다.


#120 FR01/09/10/16·NFR01/02/05/07/08·TS17/18/20/21/31/35/36 → [ADR0056](../adr/0056-daily-time-persistence-and-export.md) → rights-first nullable time/실PG sealed first completion·불변/본인export → [검증120](../verification/120-daily-time-persistence-and-export.md). 공개 시간HTTP·두 순위/UI·상위19는 후속이다.


#122 FR01/09/10/16·NFR01/02/05/07/08·TS17/18/20/21/31/35/36 → [ADR0057](../adr/0057-authenticated-daily-timing-http.md) → 인증 start/heartbeat/submit-timed·same authority/reap/current proof/실PG → [검증122](../verification/122-authenticated-daily-timing-http.md). 순위/UI/상위19는 후속이다.


#124 FR01/09/10/16·NFR01/02/05/07/08·TS17/18/20/21/31/35/36 → [ADR0058](../adr/0058-public-daily-rankings.md) → public bounded 두 순위/accepted metadata·현재 nickname·secret 제외·실PG snapshot/index·[검증124](../verification/124-public-daily-rankings.md). 명시 공식 웹/대기/상위19 통합은 후속이다.


#126 FR09/10·NFR01/02/05/07/08·TS17/18/21/31/35/36 → [ADR0059](../adr/0059-accepted-daily-core-initialization.md) → accepted snapshot Native/WASM/worker·Rust 단일 규칙·strict16KiB/no fallback·golden parity → [검증126](../verification/126-accepted-daily-core-initialization.md). 공식 웹/대기·상위19 통합은 후속이다.

TS17/18 확장(#126): bundled와 다른 accepted countdown/hash/seed의 Rust golden으로 Native·실WASM·production worker 전체 view/입력/완료 replay를 비교한다. unknown field/손상 hash/버전/16KiB 초과는 거절하고 실패 뒤 snapshot은 not_started·다음 정상 init는 가능하다. 기존 두 인자 init와 offline 연습을 유지한다.


TS17/18/20/21/29/31/35/36 확장(#128): [ADR0060](../adr/0060-daily-client-and-explicit-pending.md)의 strict daily client·현재 account/revision 전후 proof와 terminal 만료/unsupported post-proof를 확인한다. 실제 production HTTPS PC/mobile8언어 설정에서 WASM 완료 후보→명시 계정 확인→서버 첫 완료/Practice·실패 보존·개인 기록 유지, 별도 실제HTTPS typed start/heartbeat/own/익명 두 순위를 검사한다. 자동 flush·공식 UI 완료를 가정하지 않는다.


#130 FR09/10/11·NFR01/02/05/06/08·TS17/18/19/20/21/26/36 → [ADR0061](../adr/0061-daily-rankings-ui.md) → 공개 두 순위 lazy 화면/무쿠키·UTC 새로고침·역순/취소/offline·미발행/빈/실패 구분·8언어 semantic table/긴 nickname → [검증130](../verification/130-daily-rankings-ui.md). 공식 시작/controller·본인 receipt/공유·상위19는 후속이다.


#132 FR01/09/10/11/16·NFR01/02/05/06/07/08·TS17/18/19/20/21/22/26/29/31/35/36 → [ADR0062](../adr/0062-official-daily-flow.md) → 명시 공식 시작/accepted WASM·단일 heartbeat/owner 수명·unverified 후보 선보존/같은 attempt 재시도·본인 receipt/두 시간 신뢰·allowlist 공유/8언어 → [검증132](../verification/132-official-daily-flow.md). 실제 OAuth/운영/사람 검증과 상위19 전체 출구는 별도다.

TS17/18/20/21/35 확장(#132): 현재 계정 확인 전 start=0, 중복 start=1, 응답 소실 재시도는 같은 attempt. 서버 accepted metadata/rules가 WASM 초기화 결과와 다르면 board/제출=0. heartbeat는 단일 순번·무중첩이며 완료는 진행 중 ACK를 기다린다. ACK null/실패 후 ticket=null, 화면 이탈/계정 revision/연결 변경은 즉시 abort·private view 제거, 늦은 이전 heartbeat/결과는 새 run을 변경하지 않는다. 종료 미완료는 저장/제출=0. 완료는 unverified first 후보를 먼저 저장하고 실패 시 보존·명시 재시도는 같은 attempt다. 다른 첫 local 후보는 삭제하지 않는다. Practice는 원래 서버 receipt를 표시한다. 본인 조회는 정상 없음/실패를 구분한다.

TS19/22/26/36 확장(#132): 공식 완료 시간은 receipt의 nullable verified_elapsed_ms만 사용하고 로컬 시간을 개인 참고로 표시한다. text/PNG는 미검증·완료 검증·시간 검증을 일관되게 표시한다. allowlist 밖 nickname/account/attempt/ticket/completion ID·seed/hash/정답/입력은 공유하지 않고 공유 동작은 daily 요청을 발생시키지 않는다. PC1440/mobile390의 실제 HTTPS/PG/WASM 보드 입력→첫 완료/Practice→503 보존·명시 재시도→8언어 본인 조회와 수평 넘침·offline private 결과 제거를 확인한다.


#134 FR16·NFR01/02/05/07/08·TS20/21/22/24/29/31/34/35/36 → [ADR0063](../adr/0063-event-domain-contracts.md) → closed client/server 권위·목적별 consent/identity·30일/달력13개월·bounded NSM/D7/완결성·domain95 누락 방지 → [검증134](../verification/134-event-domain-contracts.md). 실제 PG 수집/retention·권리/운영·상위20은 후속이다.

TS29 확장(#134): 최대1024byte/closed client draft에서 민감·server/essential/임의 payload 필드는 거절한다. analytics와 advertising의 각각 유효 consent revision이 발생/현재를 덮어야 한다. 같은 event_id의 동일내용/충돌/다른 ID, 원래시각30일·calendar13개월 정확 만료/월말/역행을 구분한다. NSM은 월요일주 clear/timeout 1:1 account+match2회이며 bot 포함·포기/abort/daily/client 제외; 양쪽 참여자/중복/오염/부분주·10000상한을 검증한다. D7은 별도 첫 정상 날짜 snapshot·정확+7UTC일/미성숙null·분모0/불완전 입력 거절·삭제 후 식별 입력 제거를 확인한다. 실제 event PG/삭제/HTTP/운영은 다음 하위 검증이다.


#136 FR01/16·NFR01/02/05/07/08·TS20/21/29/31/34/35/36 → [ADR0064](../adr/0064-essential-event-store-and-rights.md) → committed PG source/동일account 잠금·불변 dedup/원래30일·default-deny optional·본인 typed event export/삭제 → [검증136](../verification/136-essential-event-store-and-rights.md). 자동 수집/물리retention/집계·상위20은 후속이다.

TS29 확장(#136): 실제 committed 본인 인간 match/daily source로만 필수 event를 만든다. 같은ID/다른내용 경합·새ID의 같은source 재시도는 accepted ID/received_at을 보존한다. account삭제와 같은 잠금/transaction·삭제/만료뒤재전송 비복원·exact30일·rollback/timeout·불변/essential CHECK를 실제PG로 확인한다. 기존 인증 export의 event 스트림은 own51개 페이지·현재session/recent auth·전후 proof·취소·strict 최소 DTO/32MiB 안에서 실제HTTPS 다운로드에 포함된다. optional runtime 수집은0이다.

#138 FR01/09/14/16·NFR01/02/05/07/08·TS15/16/17/18/20/21/29/31/35/36 → [ADR0065](../adr/0065-atomic-essential-event-producers.md) → 원본 transaction 안의 자동 essential 수집·UUID순 계정 잠금·공동 rollback·재전송 비복원·unknown OS 복구/본인 HTTPS 다운로드 → [검증138](../verification/138-atomic-essential-event-producers.md). 물리 보존·first-normal/NSM/D7·상위20 통합 출구는 후속이다.

TS29 확장(#138): direct/runtime·journal finish/unknown recovery·daily 첫 official은 source와 essential event를 함께 commit한다. 실제 PG event INSERT 실패는 source/active 삭제까지 rollback한다. 같은 source/attempt 재전송과 Practice는 최초 ID/received_at을 유지하고 raw 삭제/정확30일·원본90일 만료 후 복원하지 않는다. 반대 좌석 계정·batch는 UUID순 잠금이며 계정 삭제는 bot/인간 재생성 없이 반영된다. 실제 main OS kill/restart와 수동 capture 없는 HTTPS PC/mobile8언어 own 다운로드를 확인한다.

#140 FR01/16·NFR01/02/05/07/08·TS20/21/29/31/35 → [ADR0066](../adr/0066-bounded-raw-event-retention.md) → 원래 occurred_at30일 물리 삭제·500개/transaction·2초/SKIP LOCKED·최소 row-lock 권한·main 별도 maintenance → [검증140](../verification/140-bounded-raw-event-retention.md). calendar13개월/first-normal/NSM·D7와상위20은후속이다.

TS29 확장(#140): 실제 PG의 exact30일·received 비연장·500+1/동시·잠긴row skip/backlog·SQLfail/Clockrollback/2초·취소 rollback·본인 논리 만료와 물리 삭제·원본/권리 유지·raw retry 비복원을 확인한다. main즉시/60초·cycle8batch/2초·진전0/skip tick/겹침·최소DML과physicalstartup출구를확인한다. 현재없는anonymous13개월집계는완료로계산하지않는다.


#143 FR01/16·NFR01/02/05/07/08·TS20/21/29/31/34/35/36 → [ADR0067](../adr/0067-authoritative-metric-snapshots.md) → collection/30일 경계·RR/2초/10000입력·권위 first-normal 근거·NSM/D7 불완전 거절 → [검증143](../verification/143-authoritative-metric-snapshots.md). 새 개인 first marker/평생flag 없이 기존 source90일을 사용하며 익명 저장/13개월/운영조회·상위20 감사는 후속이다.


#145 FR01/16·NFR01/02/05/07/08·TS20/21/29/31/34/35/36 → [ADR0068](../adr/0068-anonymous-metrics-retention-and-operations.md) → same-RR 익명 scalar 집계·원래13달력개월/500개2초 정리·자동주기/로컬운영·계정삭제·상위20 감사 → [검증145](../verification/145-anonymous-metrics-retention-and-operations.md). optional21/외부42·25/26·27·미확정83/141은 별도다.


#148 NFR01/02/05/07/08 · TS15/16/20/21/31 → [ADR0069](../adr/0069-main-preparation-diagnostics.md) → 실제 child별 고유 로그·단조 단계/실패 상태·재시작 보존 → [검증148](../verification/148-main-preparation-diagnostics.md). 상위141의 미확정 준비 원인과 진단 보존을 구분한다.


#150 FR01/10/11/13·NFR01/02/06/07/08·TS18/19/24/26/31/35/36 → [ADR0070](../adr/0070-technical-storage-notice.md) → 읽기 전용8언어 실제 저장소/쿠키·footer/route/dialog·유지/삭제/메모리 대체 → [검증150](../verification/150-technical-storage-notice.md). 실제 CMP/전체 정책/외부147·상위21과83/141은 별도다.

TS24/19/18 확장(#150): footer에서 언어별 저장소 고지에 접근하고 설정 dialog와 같은 실제 범주/기간/삭제 안내를 읽는다. 읽기/닫기가 login/동의/전송/삭제를 실행하지 않고,30개 로컬 결과/명시 제출·5분/30일 쿠키·기록/캐시/로그아웃/계정삭제 범위·메모리 대체가8언어에 존재한다. PC/mobile/360px·keyboard/초점 복구·저장소 차단/서버 실패 뒤 로컬 연습이 계속 가능하다. 실제CMP/법적 정책 검수는147/21의 후속이다.


#152 FR11/13·NFR01/02/06/07/08·TS19/24/26/29/31/36 → [ADR0071](../adr/0071-purpose-consent-port.md) → 목적별 닫힌 current CMP port/재열기수명·8언어설정·실행앱미설정차단 → [검증152](../verification/152-purpose-consent-port.md). 실제CMP/계정탭/서버동의·선택수집/외부147·상위21은후속이다.

TS24 확장(#152): fake CMP valid analytics-only/advertising-only→해당목적만허용,unknown/denied/error/예외/추가필드/잘못된boolean→양쪽차단. 현재read가철회되면예전view와재열기late Promise가허용을복원하지않는다. pending/중복/실패/취소/dispose·구독오류를검사한다.8언어설정과실행앱unavailable/저장소차단·503/PCmobile에서추가선택요청없이실제로컬연습을계속한다. 실제CMP/계정탭/서버transaction통합은21/147후속이다.


#154 FR10·NFR01/02/05/07/08·TS18/20/26 → [ADR0072](../adr/0072-offline-fetch-diagnostics.md) → fixture 전용 respondWith/handled 독립 수명·원본 Promise/stream/반환/예외 보존·닫힌 진단 → [검증154](../verification/154-offline-fetch-diagnostics.md). 제품 원인 #83은 별도 OPEN이다.

TS18 확장(#154): 두 요청 응답 Promise의 역순 완료/거절과 handled·waitUntil의 독립 pending을 구분한다. native 동기 예외/반환값·원래 Promise를 보존하고 열린 ReadableStream body를 소비하거나 잠그지 않는다. 로그에 원시 URL/query/header/body가 없고 단조 시간을 유지한다. 실제 WASM PC/mobile 업데이트·캐시 삭제의 원래 인수 조건을 유지하며 성공 반복은 #83 원인 해결을 의미하지 않는다.


#156 FR01/10/11/13·NFR01/02/05/06/07/08·TS18/19/20/24/26/29/31/35/36 → [ADR0073](../adr/0073-consent-context-and-expiry.md) → 메모리 현재 context/revision/만료·계정탭/복원 취소·시계/타이머 정리 → [검증156](../verification/156-consent-context-and-expiry.md). 실제 CMP/서버 proof/선택 수집·외부147/상위21은 후속이다.

TS24 확장(#156): 검증 시작 때 capture한 현재 token과 목적별 receipt만 허용한다. 만료 경계·미래/잘못된 시각·시계 역행·낮은 revision/같은 revision 변조를 차단하고 이전 grant를 복원하지 않는다. 계정/세션/peer/숨김/복원·명시 reopen에서 token교체/진행abort/late차단을 관측한다. 만료 UI갱신·긴timer/cleanup·채널불가failclosed와무계정게임무차단을 확인한다.
