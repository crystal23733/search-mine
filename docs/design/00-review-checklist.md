# 설계 리뷰·승인 체크리스트

> 2026-10-06 · **사용자 승인 대기** · M0 문서 PR에 연결.
> 기획/설계 완료는 제품 구현 승인과 다르다. [승인 상태](../workflow/design-approval.json)

## 검토 결과

- 기획21개·상세설계16개·ADR8개·TS01~30 작성.
- SRP: Board truth / PlayerBoard visibility / RuleEngine transition / LieValidator proof / BotPolicy decision 책임 분리. DB·시간·전송은 포트와 어댑터로 분리.
- 규칙: 동률·타임아웃·기절 경계·중복 명령·마지막 안전칸과 공격·끊김·서버 장애 정의.
- 보안: secret online seed·Board와 public DTO 분리, STRIDE·Origin·CSRF·rate·내부 DB·백업 정의.
- 광고/i18n: 8언어와 fallback·URL/SEO, CMP 거부/철회, 3판 cap·플레이 광고0·SDK 실패 경로 정의.
- 공정성/성능/수익: **미검증 가정**과 측정 계획을 구분. 알고리즘 증명·미니 PC 성적·Google 승인은 향후 과제.
- 링크·Mermaid·스킬 검사는 초기 PR의 검증 결과를 아래에 기록한다. 제품 테스트는 아직 작성/실행하지 않았다.

## 요구사항 → 설계 → 시나리오 추적

| 요구 | 이야기 | 상세 설계 | 테스트 시나리오 / 경계 |
|---|---|---|---|
| FR01 게스트 | US01 | [07 DB](07-database.md), [12 보안](12-security.md) | TS01/TS20 GuestService·GuestRepository |
| FR02 노게스/동일판 | US02 | [03 도메인](03-domain-model.md), [05 알고리즘](05-algorithms.md) | TS02/TS03 BoardGenerator·KnowledgeSolver |
| FR03 승패/실수 | US03 | [04 규칙](04-game-rules-spec.md) | TS04/TS05/TS14 RuleEngine·Clock |
| FR04 공격 | US04 | [04](04-game-rules-spec.md), [05](05-algorithms.md) | TS06/TS07/TS21/TS22 LieValidator·ProtocolDecoder |
| FR05 지목 | US05 | [04](04-game-rules-spec.md), [09 UX](09-screens-ux.md) | TS08/TS09 RuleEngine·InputController |
| FR06 빠른대전/봇 | US06 | [05](05-algorithms.md), [06 프로토콜](06-protocol.md) | TS10/TS11 Matchmaker·BotPolicy |
| FR07 친구방 | US07 | [06](06-protocol.md) | TS12 RoomService·GuestRepository |
| FR08 튜토리얼 | US08 | [08 client](08-client-architecture.md), [09](09-screens-ux.md) | TS13 TutorialController·StoragePort |
| FR09 데일리/공유 | US09 | [06](06-protocol.md), [07](07-database.md), [09](09-screens-ux.md) | TS17 DailyService·ReplayVerifier |
| FR10 오프라인 | US10 | [02 배포](02-deployment-network.md), [08](08-client-architecture.md) | TS18 CorePort·StoragePort·NetworkPort |
| FR11 언어/SEO | US11 | [10 i18n](10-i18n.md), [09](09-screens-ux.md) | TS19/TS26 I18nPort·Router·DOM Grid |
| FR12 광고 | US12 | [11 광고](11-ads-consent.md) | TS23/TS25 AdsPolicy·StoragePort |
| FR13 동의/정책 | US13 | [11](11-ads-consent.md), [12](12-security.md) | TS24 ConsentPort·AdsPort |
| FR14 재접속 | US14 | [04](04-game-rules-spec.md), [06](06-protocol.md) | TS15/TS16 MatchActor·Transport·Clock |
| FR15 튜닝 | US15 | [04](04-game-rules-spec.md), [15 구조](15-repo-structure.md) | TS27/TS30 RulesLoader·TypeGenerator |
| FR16 운영/이벤트 | US16 | [07](07-database.md), [16 운영](16-observability-ops.md), [metrics](../product/20-metrics.md) | TS28/TS29 BackupRunbook·EventRepository |
| NFR01 SRP/DIP | US02/US15 | [01 구조](01-architecture.md), [03](03-domain-model.md), [08](08-client-architecture.md), [15](15-repo-structure.md) | TS02/TS30 의존성·타입 경계 |
| NFR02 보안 | US01/US04 | [06](06-protocol.md), [12](12-security.md) | TS20/TS21/TS22 |
| NFR03 초기bundle/FPS | US11/US15 | [13 성능](13-performance.md) | TS26/TS27 build·실기기 |
| NFR04 LCP/입력/생성 | US02/US15 | [05](05-algorithms.md), [13](13-performance.md) | TS03/TS27 criterion·k6 |
| NFR05 TDD/coverage | 전체 | [14 테스트](14-testing-strategy.md) | TS01~TS30 / 각 포트 |
| NFR06 접근성 | US11 | [08](08-client-architecture.md), [09](09-screens-ux.md) | TS09/TS26 InputController·DOM Grid |
| NFR07 홈서버/복구 | US10/US16 | [02](02-deployment-network.md), [07](07-database.md), [16](16-observability-ops.md) | TS16/TS18/TS28 |
| NFR08 develop/승인 | US15 | [workflow](../workflow/git-workflow.md), [14](14-testing-strategy.md) | TS30 CI·approval record |

코어 시나리오는 Clock/RNG fake로 제어하고, DB/WS는 실제 통합으로, UI/광고/언어는 DI port로 테스트한다. [백로그](../workflow/backlog.md)는 각 작업의 시나리오를 연결한다.

## 사용자가 승인할 구체 제안

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

## 승인 방법

사용자가 초기 PR의 위 제안들을 검토하고 명시적으로 설계를 승인하면, 별도 승인 이슈에서 design-approval.json과 ADR 상태를 승인 근거·검토 commit으로 갱신한 PR을 먼저 병합한다. 단순 초기화 PR 병합만으로 알고리즘 가정이 입증되었다고 표시하지 않는다.
