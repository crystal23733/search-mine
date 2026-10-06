# 19. 테스트 시나리오

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
