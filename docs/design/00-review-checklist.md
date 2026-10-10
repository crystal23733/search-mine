# 설계 리뷰·승인 체크리스트

> 아래 초기 검토 결과와 검사 기록은 M0 당시 기록이다. 현재 작업 권한은 [최신 사용자 위임](../workflow/approval-record.md)과 [AGENTS](../../AGENTS.md)가 우선한다. #4·#5는 병합했고 #6은 [공개 후보 전략 ADR0010](../adr/0010-public-certified-attack-strategy.md)과 실제 검증 결과를 추가한다. 제품 코드가 없다는 아래 문장은 초기 시점의 기록이다.

> 2026-10-06 · **사용자 변경·구현 시작 지시 반영, PR 병합 대기** · M0 문서 PR에 연결.
> [지시 근거](../workflow/approval-record.md)를 기록했다. 공정성/성능 검증과 PR 병합 승인은 별도다. [승인 상태](../workflow/design-approval.json)

## 검토 결과

- 기획21개·상세설계18개·ADR9개·TS01~36 작성; 원본 화면18개 연결.
- SRP: Board truth / PlayerBoard visibility / RuleEngine transition / LieValidator proof / BotPolicy decision 책임 분리. DB·시간·전송은 포트와 어댑터로 분리.
- 규칙: 동률·타임아웃·기절 경계·중복 명령·마지막 안전칸과 공격·끊김·서버 장애 정의.
- 보안: secret online seed·Board와 public DTO 분리, STRIDE·Origin·CSRF·rate·내부 DB·백업 정의.
- 광고/i18n: 8언어와 fallback·URL/SEO, CMP 거부/철회, 3판 cap·플레이 광고0·SDK 실패 경로 정의.
- 공정성/성능/수익: **미검증 가정**과 측정 계획을 구분. 알고리즘 증명·미니 PC 성적·Google 승인은 향후 과제.
- 링크·Mermaid·스킬 검사는 초기 PR의 검증 결과를 아래에 기록한다. 제품 테스트는 아직 작성/실행하지 않았다.

## 요구사항 → 설계 → 시나리오 추적

#82: FR14/16·NFR01/02/05/07 → 설계06/07/12·[ADR0039](../adr/0039-authenticated-personal-result-read.md) → TS15/16/20/21/31/35/36 → [검증82](../verification/82-authenticated-personal-result-read.md). 최소 결과·본인 참여·전후 철회·uniform404·90일 조회·실DB/HTTP를 확인한다.

#84 NFR05/08·TS18/26/30 → [테스트 경계](14-testing-strategy.md) → 실제 App/권한/256셀을 유지한 준비·조회 시간 관측과 단위 시나리오 분리 → [검증 기록](../verification/84-unit-arrangement-observation.md). 제품 lazy route/cold browser 계약과 테스트 상한은 유지한다.

#83/#85 FR10/NFR05/07·TS18/20/26 → [ADR0018](../adr/0018-public-offline-cache-and-safe-update.md) → production 두 탭 업데이트의 연속 observer/native lifecycle·항상 보존하는 CI trace. #74/#80의 간헐 실패는 원인 미확정이며 timeout/retry 완화 없이 분리한다. #85 진단 보존과 미완료 #83 제품 조사를 구분해 [진단 기록](../verification/85-offline-update-observation.md)을 갱신한다.

#18/#76의 FR14/NFR01/02/05/06·TS15/16/20/21/26은 [ADR0036](../adr/0036-opponent-reconnect-grace.md)의 Rust 상대 유예→공개 view/Visible 정규화→strict decode/8언어 표시로 추적한다.0은 서버 결과 대기이며 자체 판정하지 않는다. 자동 재접속·재시작 영속 복구는 후속18 출구다.

#18/#74의 FR14/NFR01/02·TS15/16/20/21은 [ADR0035](../adr/0035-reconnect-command-cursor.md)의 Rust 본인 입력 cursor→snapshot→새 controller와 새 epoch cached ACK/original revision·실제 TCP WS/HTTPS reload로 추적한다. 자동 backoff·grace 공개 상태·서버 재시작 영속 복구는 후속18 출구다.

#17/#66의 FR01/07/08/11·TS01/12/13/19/20/31~36은 [ADR0031](../adr/0031-oauth-invitation-return.md), auth HTTP/service·실제 PostgreSQL·HTTPS browser로 추적한다. 서버 거래의 canonical 초대/locale·중첩/역순·일회 소비/expiry·nickname/tutorial·저장소 거부를 검증하며 실제 온라인 화면은 다음 출구다.

#17/#52의 FR06/07·TS10/12는 [ADR0024](../adr/0024-atomic-lobby-and-online-composition.md)와 `crates/server/tests/lobby_policy.rs`의 IO 없는 경계 시험을 따른다. 실제 보드/봇·인증 전송/초대 거래·browser 출구는 후속 #17 하위 작업에서 검증한다.

#54의 FR02/06/07·TS02/10/11/12/21은 [ADR0025](../adr/0025-bounded-private-certified-board-pool.md)와 `crates/server/tests/board_pool.rs`의 실제 생성·통제된 blocking source·capacity/수명 검사로 추적한다. pool만으로 실제 대전 조립을 완료했다고 보고하지 않는다.

#16의 FR02~05/15·TS14/20/21/22는 [ADR0023](../adr/0023-authoritative-match-actor.md), `crates/protocol/tests/online.rs`, `crates/server/tests/online_state.rs`, `online_authority.rs`와 실제 PostgreSQL/WS 시험으로 추적한다. 승인된 계정 철회 observer를 actor에 연결하며 구현 중 결과를 완료 증거와 구분한다.

[#16 실행 근거](../verification/16-authoritative-match-actor.md)는 실DB23·실제TCP WS9·actor4·state11의 결과, Red→Green과 coverage gate를 기록한다. 매칭17/화면18·실제 키42·운영25/26의 출구를 완료로 표현하지 않는다.

| 요구 | 이야기 | 상세 설계 | 테스트 시나리오 / 경계 |
|---|---|---|---|
| FR01 OAuth·최소 정보 | US01 | [07 DB](07-database.md), [17 인증](17-auth-privacy.md), [18 화면](18-design-layout.md) | TS01/20/31/32/33/34/35/36 AuthService·OAuthProvider·AccountRepository |
| FR01 #44 제공자/HTTP | US01 | [ADR0020](../adr/0020-oauth-providers-and-http.md), [06 프로토콜](06-protocol.md) | TS01/20/31/32/33: 최소 authorize·signed JWT·CSRF/Origin·one-use callback·session revision·실DB |
| FR01/16 #45 권리/철회 | US01 | [ADR0021](../adr/0021-account-rights-and-apple-revocation.md), [17 인증](17-auth-privacy.md) | TS20/29/34/35: 최근 인증·명시적 연결·원자 삭제·last provider·signed notifications·실DB |
| FR01/10/11/16 #46 계정 화면 | US01/US11 | [ADR0022](../adr/0022-web-oauth-account-ui.md), [18 화면](18-design-layout.md) | TS01/19/26/31/34/35/36: 최소 요청·메모리 세션·generation·계정 권리 확인·8언어/실DB HTTP E2E |
| FR02 노게스/동일판 | US02 | [03 도메인](03-domain-model.md), [05 알고리즘](05-algorithms.md) | TS02/TS03 BoardGenerator·KnowledgeSolver |
| FR03 승패/실수 | US03 | [04 규칙](04-game-rules-spec.md) | TS04/TS05/TS14 RuleEngine·Clock |
| FR04 공격 | US04 | [04](04-game-rules-spec.md), [05](05-algorithms.md) | TS06/TS07/TS21/TS22 LieValidator·ProtocolDecoder |
| FR05 지목 | US05 | [04](04-game-rules-spec.md), [09 UX](09-screens-ux.md) | TS08/TS09 RuleEngine·InputController |
| FR06 빠른대전/봇 | US06 | [05](05-algorithms.md), [06 프로토콜](06-protocol.md) | TS10/TS11 Matchmaker·BotPolicy |
| FR07 친구방 | US07 | [06](06-protocol.md) | TS12 RoomService·AccountRepository |
| FR08 튜토리얼 | US08 | [08 client](08-client-architecture.md), [09](09-screens-ux.md) | TS13 TutorialController·StoragePort |
| FR09 데일리/공유 | US09 | [06](06-protocol.md), [07](07-database.md), [09](09-screens-ux.md) | TS17 DailyService·ReplayVerifier |
| FR10 오프라인 | US10 | [02 배포](02-deployment-network.md), [08](08-client-architecture.md) | TS18 CorePort·StoragePort·NetworkPort |
| FR11 언어/SEO | US11 | [10 i18n](10-i18n.md), [09](09-screens-ux.md), [18 화면](18-design-layout.md) | TS19/26/36 I18nPort·Router·DOM Grid |
| FR12 광고 | US12 | [11 광고](11-ads-consent.md) | TS23/TS25 AdsPolicy·StoragePort |
| FR13 동의/정책 | US13 | [11](11-ads-consent.md), [12](12-security.md) | TS24 ConsentPort·AdsPort |
| FR14 재접속 | US14 | [04](04-game-rules-spec.md), [06](06-protocol.md) | TS15/TS16 MatchActor·Transport·Clock |
| FR15 튜닝 | US15 | [04](04-game-rules-spec.md), [15 구조](15-repo-structure.md) | TS27/TS30 RulesLoader·TypeGenerator |
| FR16 운영/이벤트 | US16 | [07](07-database.md), [16 운영](16-observability-ops.md), [metrics](../product/20-metrics.md) | TS28/TS29 BackupRunbook·EventRepository |
| NFR01 SRP/DIP | US02/US15 | [01 구조](01-architecture.md), [03](03-domain-model.md), [08](08-client-architecture.md), [15](15-repo-structure.md) | TS02/TS30 의존성·타입 경계 |
| NFR02 보안 | US01/US04 | [06](06-protocol.md), [12](12-security.md), [17](17-auth-privacy.md) | TS20/21/22/31~35 |
| NFR03 초기bundle/FPS | US11/US15 | [13 성능](13-performance.md) | TS26/TS27 build·실기기 |
| NFR04 LCP/입력/생성 | US02/US15 | [05](05-algorithms.md), [13](13-performance.md) | TS03/TS27 criterion·k6 |
| NFR05 TDD/coverage | 전체 | [14 테스트](14-testing-strategy.md) | TS01~TS36 / 각 포트 |
| NFR06 접근성 | US11 | [08](08-client-architecture.md), [09](09-screens-ux.md) | TS09/TS26 InputController·DOM Grid |
| NFR07 홈서버/복구 | US10/US16 | [02](02-deployment-network.md), [07](07-database.md), [16](16-observability-ops.md) | TS16/TS18/TS28 |
| NFR08 develop/승인 | US15 | [workflow](../workflow/git-workflow.md), [14](14-testing-strategy.md) | TS30 CI·approval record |

코어 시나리오는 Clock/RNG fake로 제어하고, DB/WS는 실제 통합으로, UI/광고/언어는 DI port로 테스트한다. [백로그](../workflow/backlog.md)는 각 작업의 시나리오를 연결한다.

OAuth 기반 FR01/NFR02·TS01/20/31/32/35 → [ADR0019](../adr/0019-auth-foundation-and-delivery.md) → #43(AuthStore/CredentialVault). 이후 #44 provider/HTTP, #45 계정 권리, #46 UI를 순차 병합하고 #42 실제 계정 검수는 별도 출시 조건이다.

## 구현 기준과 남은 검증

1. **공정성:** 모든 관측 모델의 안전 진행·lie 식별 증거를 요구하고 불명확/timeout 후보는 거절. 실현 가능성은 M1 E4에서 평가하며 반례가 있으면 출시 차단.
2. **규칙:** 고정 zero 오프닝, 시작 gauge0, capacity20·새 안전칸당1, 자동 서버 target 선택, 실패 게이지 보존, chord 미제공. 경고는 공격 정보가 아닌 공통 분위기 연출.
3. **판정:** 서버 수신 순서, timeout 동률 draw, 30초 재접속 유예, 모두 끊기면 abandon draw, 서버 장애 abort.
4. **데일리:** offline replay는 검증 완료/오류 순위, server start·heartbeat가 있는 attempt만 시간 순위. 첫 검증 완료만 공식 기록.
5. **성능:** JS200KiB를 초기 critical path로 정의하고 game 청크/WASM을 별도 예산으로 측정. 전체 game JS200KiB 충족은 미확인.
6. **작업 흐름:** develop 기본, Conventional Commits, 이슈별 검토 PR, 1인 공식 review count0+필수checks+사용자 수동 검토, 승인 전 제품 변경 CI 차단.

## 운영 전에 채워야 할 정보

미니 PC 사양/OS·회선 업로드·소비전력·별도 백업 매체·주당 시간·테스트기기·도메인·실제 운영자 연락처·미성년자 방침·Google 계정 승인·인증 CMP·8언어 검수자. 문서 작성과 승인은 가능하지만 이 값 없이 배포·법적 문구·성능 결과를 확정하지 않는다.

## 검사 기록

- `python scripts/validate_docs.py`: Markdown 66개, 기획21개·설계16개·TS30개 정의/추적·상대 링크 검사 통과.
- `node scripts/check_mermaid.mjs .tmp/docs-tools`: Mermaid 31개 실제 parser 검사, 오류0.
- system skill-creator의 `quick_validate.py`를 `python -X utf8`로 실행: 공통3개+Claude entry point3개 모두 통과.
- `git diff --check`: 통과. GitHub Actions의 docs/contribution-gate는 PR 생성 뒤 확인한다.

제품 코드는 0줄이며 제품 테스트·부하·공정성 실험은 승인 후 수행한다. Mermaid parse는 문법 검증이고 UI 시각 완성도나 제품 동작의 검증을 뜻하지 않는다.

## 구현 시작과 병합 조건

사용자의 OAuth 변경과 개발 시작 지시를 별도 #3 설계 PR에 기록했다. #29 초기 설정과 #3 PR은 검사 후 사용자 병합 승인을 받아 develop에 순서대로 반영한다. 그 뒤 #4 제품 작업을 시작한다. 이후 코드 PR마다 사용자 검토/병합을 거친다. 설계 기록을 공정성·법적 준수·성능 입증으로 표현하지 않는다.

## OAuth·화면 변경 추적

TS31 → 최소 권한/claim·FR01/NFR02, TS32 → 거래 바인딩·NFR02, TS33 → 4개 어댑터/확장 검증·FR01/NFR01/02, TS34 → 명시적 계정 연결·FR01, TS35 → 내보내기/삭제·FR01/16, TS36 → 무계정 연습/18개 화면·FR01/08/10/11/13. 상세 계약은 17/18 설계와 ADR0009, 구현 이슈는 #10~15/#20/#21이다.

초기 검증 기록은 PR #29의 813d13f 기준이다. 이 변경의 문서/Mermaid 결과는 #3 PR에 별도 기록한다.

#56의 FR06·TS10/11/21은 [ADR0026](../adr/0026-public-observation-online-bot-driver.md)와 `crates/server/tests/online_bot.rs`의 실제 core 봇·통제된 blocking 입력·늦은 완료/실패·public projection 검사로 추적한다.

#58의 FR02/06/07·TS02/10/11/12/21은 [ADR0027](../adr/0027-atomic-lobby-match-composition.md)와 `crates/core/tests/prepared_engine.rs`·`crates/server/tests/lobby_service.rs`의 준비/시각 시작·actual blocking 수명·취소/generation·실제 registry/core bot snapshot으로 추적한다.

#60의 FR01/02/06/07/14/16·NFR02·TS10/12/20/21/31~36은 [ADR0028](../adr/0028-session-bound-lobby-admission.md)과 authority/로비 행동·실제 PostgreSQL 철회 검사로 추적한다. 공개 HTTP/main·배정 후 최초 WS 수명·OAuth 초대/화면은 별도 후속 출구다.

#62의 FR02/06/07/14/16·NFR02·TS10/12/15/16/20/21/35는 [ADR0029](../adr/0029-initial-match-connection-lifecycle.md)의 core 시작 전 취소와 admitted actor의 첫 연결/권한 이전·지연 ingress·취소 결과/소유권 반환으로 추적한다.

#64의 FR01/02/06/07/14/16·NFR02·TS10/12/20/21/31~36은 [ADR0030](../adr/0030-authenticated-lobby-http-runtime.md)의 닫힌 Rust→TS 로비 계약·인증 HTTP/rate/세션 generation·실제 실행 조립으로 추적한다.

#66 검증에서 발견한 FR10·TS18/36의 다중 탭 업데이트 회귀는 [ADR0018](../adr/0018-public-offline-cache-and-safe-update.md)와 `tests/e2e/offline.spec.ts`의 게임 중 차단·idle 승인 뒤 실제 두 탭 reload로 추적한다. 요청 수락과 controller 변경을 분리하고 기존 lock/시간 제한을 유지한다.

#68의 FR01/06/07/14·NFR01/02/05·TS10/12/15/20/31/36은 [ADR0032](../adr/0032-session-bound-web-requests.md)의 전/후 권한 확인·메모리 fresh proof·병렬 상한/전체 deadline·취소/late reply·domain 오류 보존을 실제 AuthPort 행동 검사로 추적한다. 로비/WS 화면의 실제 HTTPS 출구는 별도 #17 후속이다.

#70의 FR01~06/10/11/14/15·NFR01/02/05/06·TS02/04~11/14~16/18~22/26/31/36은 [ADR0033](../adr/0033-online-quick-match-ui.md)의 실제 빠른 대전/공개 보드·bounded 세션 HTTP/WS·입력/결과·8언어와 Rust/PostgreSQL/HTTPS 출구로 추적한다. 친구 방 UI·자동 재접속·실제 키/하드웨어/사람 검수는 별도다.

#72의 FR01/02/06/07/08/10/11/14·NFR01/02/05/06·TS02/10/12/13/15/18~22/26/31/36은 [ADR0034](../adr/0034-friends-room-ui.md)의 명시 참가/초대 복귀·현재 방 identity/ready·seat 이동/만료·공유된 실제 온라인 화면과 Rust/SQL/HTTPS/WS 출구로 추적한다. 자동 재접속 #18은 별도다.

#78 FR01/14/NFR01/02/05 · TS15/16/20/31/35/36 → [ADR0037](../adr/0037-session-recovery-candidate.md) → AuthPort/SessionRecovery/HTTP proof 분류·브라우저 lifecycle와 실제 SQL/HTTPS 권한 차단/동일 세션 복구. WS backoff/미확인 명령은 다음18 하위 작업이다.

#80 FR14/NFR01/02/05/06 · TS15/16/20/21/26/31/36 → [ADR0038](../adr/0038-bounded-online-reconnect.md) → BoundedReconnect/OnlineController/OnlineEntry·권한 없는 후보/단조 기한/메모리 원본 명령·실제 WS/PG/HTTPS. 영속 재시작·결과 조회는 후속18이다.

#89 FR11/14/16·NFR01/02/05/06/08·TS15/16/20/21/26/31/35/36 → [ADR0040](../adr/0040-personal-result-web.md) → strict 최소 decoder/인증 HTTP/OnlineController/본인 결과 Atomic UI·actual PG/HTTPS. 선행82 병합·종료, 영속 journal/재시작 후속18.

#91 FR14·NFR05/08·TS15/16/20/26/31/35/36 → [ADR0041](../adr/0041-auth-recovery-response-observation.md) → production HTTPS 응답의 one-use 관측/전달·철회 후 UI/쓰기0/독립 proof·[검증91](../verification/91-auth-recovery-observation.md). timeout/retry와 제품 권한 변경 없음.

#93 FR11/14/16·NFR01/02/05/06/07/08·TS15/16/20/21/26/31/35/36 → [ADR0042](../adr/0042-unknown-result-details.md) → forward migration/reader/project·Rust→TS·strict decoder·8언어 최소 결과 UI·[실제 검증93](../verification/93-unknown-result-details.md). 선행91/PR92 병합·종료, 실제 journal/startup/processkill·새로고침 발견은 후속18이다.

#95 FR11/14/16·NFR05/08·TS15/16/20/21/26/31/35/36 → [ADR0043](../adr/0043-result-response-observation.md) → one-use 실제 결과 HTTP 응답 관측/동일 전달·known/unknown PC/mobile 반복·[검증95](../verification/95-result-response-observation.md). 제품/기한/SW/timeout/retry 변경 없음.

#97 FR14/16·NFR01/02/05/07/08·TS15/16/20/21/31/35/36 → [ADR0044](../adr/0044-online-storage-owner.md) → 같은 raw 연결의 session lock/write·bounded 큐/2초 deadline·owner loss ready/프로세스 종료·실제 PG/HTTP 검증 → [검증97](../verification/97-online-storage-owner.md). 기존 인스턴스를 종료한 뒤 단일 새 인스턴스로 교체하며 journal/startup abort/OS kill-restart 결과 복구는 다음 작업이다.
