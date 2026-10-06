# 19. 테스트 시나리오

> 적용: pm-execution:test-scenarios · 입력: [Stories](17-stories.md), [PRD](16-PRD.md)
> 설계 단계 시나리오이며 테스트 코드를 실행했다는 뜻이 아니다. 각 항목은 목적·전제·역할·행동·예상 결과·경계를 함께 정의한다.

## TS01: 게스트 시작·삭제 (US01)

- 목적: 게스트 시작·삭제의 관찰 가능한 행동을 보장한다.
- 전제·역할: 고정 Clock과 세션 저장소, 방문자.
- 단계와 각 결과: 닉네임을 제출해 쿠키를 받는다 → 신원 유지; 삭제한다 → 세션 철회; 재사용한다 → 거절.
- 최종 기대·예외: HttpOnly·Secure·유효성·NFC·1/20/21 grapheme 경계 확인.
- 테스트 경계·계층: GuestService / GuestRepository / 통합+E2E.

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
- 전제·역할: host/friend/third guest, 만료 clock.
- 단계와 각 결과: create→code/link; friend join→2seat; third join→full; 만료 후 join→expired.
- 최종 기대·예외: room code가 player session을 대체하지 않음, 중복 탭·ready 경계.
- 테스트 경계·계층: RoomService / GuestRepository / 통합+E2E.

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
- 전제·역할: 중복 event_id·30일/180일 데이터·delete guest, 운영자.
- 단계와 각 결과: 중복저장→한 건; retention 실행→삭제/집계; guest삭제→토큰철회·식별자제거.
- 최종 기대·예외: 정답·IP·토큰 미포함, 비필수 동의거부면 client 분석0, 백업잔존 고지.
- 테스트 경계·계층: EventRepository / RetentionService / 통합.

## TS30: 타입·코어 경계·TDD CI (US15)

- 목적: 타입·코어 경계·TDD CI의 관찰 가능한 행동을 보장한다.
- 전제·역할: public schema 변경·검사 pipeline, 기여자.
- 단계와 각 결과: DTO 변경→TS생성 diff; unsafe internal DTO→검사실패; Red→Green 증거 PR 첨부.
- 최종 기대·예외: 도메인 infra 의존0, coverage95/80 목표, 승인전 제품파일 CI거절.
- 테스트 경계·계층: TypeGenerator / CI / 정적검사+CI.

## 결정 사항

TS01~30을 백로그 이슈와 PR 검증 근거에 연결한다. fake는 도메인 경계에 주입하되 DB·WS 통합은 실제 구현체로 확인한다. 단위·속성·E2E·수동·성능을 서로 대체하지 않는다.

## 열린 질문

실제 저사양 모바일 모델, 미니 PC 사양, 외국어 검수자와 CMP 테스트 계정이 필요하다. 준비되지 않은 환경의 테스트는 통과로 기록하지 않는다.
