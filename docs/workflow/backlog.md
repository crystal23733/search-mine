# 마일스톤·작업 백로그

> 기본 브랜치 develop. 작업은 선행 PR 병합/이슈 종료 후 시작하고 사용자 위임에 따라 에이전트 리뷰·검사를 마친 PR을 병합한다.

## 마일스톤

| 마일스톤 | 출구 결과 |
|---|---|
| [M0 · 기획·설계·작업 체계](https://github.com/crystal23733/search-mine/milestone/1) | 초기화와 문서·작업 규칙 PR 검토, 사용자 설계 승인 기록. 승인 전 제품 구현 금지. |
| [M1 · 공정한 Rust 코어](https://github.com/crystal23733/search-mine/milestone/2) | 노게스·두 거짓말 간파·규칙·공개 관측 봇·native/WASM 재현. E4 반례0과 성능/채택률 근거. |
| [M2 · 싱글 플레이 웹 경험](https://github.com/crystal23733/search-mine/milestone/3) | Atomic UI·8언어·접근성·로컬 봇/튜토리얼·데일리·오프라인. 첫 가치와 E1/E2 검증. |
| [M3 · 권위 서버와 온라인 대전](https://github.com/crystal23733/search-mine/milestone/4) | 최소 OAuth 계정(Google·Apple·카카오·네이버)·PostgreSQL·WS·매칭/친구방·재접속·데일리 순위·자체 이벤트. 실DB/WS/E2E 통과. |
| [M4 · 동의·광고·다국어 공개 준비](https://github.com/crystal23733/search-mine/milestone/5) | Google 인증 CMP·8언어 정책·실제ads.txt·3판 빈도·SEO. 동의 거부/실패 무차단. |
| [M5 · 홈서버 운영 검증](https://github.com/crystal23733/search-mine/milestone/6) | Compose·Tunnel·DB 격리·별도 매체 백업·복구/롤백·실측 용량·회선 약관 확인. |
| [M6 · 베타와 현금 손익 검증](https://github.com/crystal23733/search-mine/milestone/7) | 플레이테스트·수치튜닝·NSM·광고 정산/전력 실측. 28일 현금 손익을 평가하고 확장은 사용자 결정. |

## 이슈

| 작업 | 단계 | 선행 작업 | 요구 / 시나리오 | 인수 조건 |
|---|---|---|---|---|
| [#2 chore: 프로젝트 초기화와 설계 리뷰 준비](https://github.com/crystal23733/search-mine/issues/2) | M0 | 없음 | NFR08 / TS30 | 원문 보존·기존 프로젝트 삭제; 기획21·설계16·ADR8·에이전트 규칙; 링크/Mermaid 검사와 develop 검토 PR |
| [#3 docs: OAuth·최소 정보·디자인 설계 수정과 구현 시작 기록](https://github.com/crystal23733/search-mine/issues/3) | M0 | [#2](https://github.com/crystal23733/search-mine/issues/2) | FR01~16, NFR01~08 / TS01~36 | 사용자의 OAuth/최소 수집/4개 제공자/디자인·개발 지시 반영; 설계18·ADR9·TS36; 승인 근거 기록; 선행 PR과 설계 수정 PR 검증·승인 후 병합 |
| [#4 chore: Rust·pnpm 워크스페이스와 제품 CI 구성](https://github.com/crystal23733/search-mine/issues/4) | M1 | [#3](https://github.com/crystal23733/search-mine/issues/3) | NFR01/05/08 / TS30 | 최신 안정 의존성 버전 인자 없이 추가·lockfile; fmt/clippy/lint/typecheck·단위/실DB/E2E 실행 경로; 공유 타입 생성 검사 |
| [#5 feat(core): 결정론 보드와 노게스 생성·솔버](https://github.com/crystal23733/search-mine/issues/5) | M1 | [#4](https://github.com/crystal23733/search-mine/issues/4) | FR02, NFR04 / TS02/03/27 | 고정 오프닝·seed 버전; 관측 기반 안전 추론과 풀이 증거; 작은 판 전수·10k seed 생성 벤치 |
| [#6 feat(core): 두 거짓말의 안전 간파 증명 검증](https://github.com/crystal23733/search-mine/issues/6) | M1 | [#5](https://github.com/crystal23733/search-mine/issues/5) | FR04 / TS06/07 | 공개 정보만으로 모델·증명 구성; 두 overlay·모호성·timeout fail closed; E4 반례0과 공격 채택률 보고 |
| [#7 feat(core): 게이지·지목·기절·승패 규칙](https://github.com/crystal23733/search-mine/issues/7) | M1 | [#6](https://github.com/crystal23733/search-mine/issues/6) | FR03/04/05/15 / TS04/05/06/08/09/14/27 | 단일 config와 시작 snapshot; 기절/동시입력/종료 경계; 중복 safe·command 효과 방지 |
| [#8 feat(core): 공개 관측으로 동작하는 3단계 봇](https://github.com/crystal23733/search-mine/issues/8) | M1 | [#7](https://github.com/crystal23733/search-mine/issues/7) | FR06 / TS11 | BotPolicy에 숨은 Board 접근 없음; 난이도 간격·지목 지연; 막힌 판에서 추측하지 않음 |
| [#9 feat(core): WASM 브리지와 public 프로토콜 타입 생성](https://github.com/crystal23733/search-mine/issues/9) | M1 | [#8](https://github.com/crystal23733/search-mine/issues/8) | NFR01/02 / TS02/22/30 | wasm-bindgen local 경계; native/WASM fixture 일치; Rust public DTO→TS, secret 타입 제외 |
| [#10 feat(web): Atomic UI·디자인 토큰·8언어 shell](https://github.com/crystal23733/search-mine/issues/10) | M2 | [#9](https://github.com/crystal23733/search-mine/issues/9) | FR11, NFR01/03/06 / TS19/26/36 | Atoms~Pages 경계·DI; en키 계약과8언어 URL; 초기 번들·모션/대비; design_layout 18개·설계18 토큰/모바일/PC 기준 |
| [#11 feat(web): Pixi 보드·터치·키보드 조작](https://github.com/crystal23733/search-mine/issues/11) | M2 | [#10](https://github.com/crystal23733/search-mine/issues/10) | FR03/04/05 / TS04/09/26 | 공개 View만 렌더; pan/zoom/long-press중복 방지; DOM grid와 접근성·fps 측정 |
| [#12 feat(web): 튜토리얼과 로컬 봇 대전](https://github.com/crystal23733/search-mine/issues/12) | M2 | [#11](https://github.com/crystal23733/search-mine/issues/11) | FR06/08 / TS11/13 | 공격/지목 각1회; skip/replay와 초대 유지; 3난이도 local·E1 관찰 준비(실측 #27/M2 출구 조건) |
| [#13 feat(web): UTC 데일리·개인 기록·공유 카드](https://github.com/crystal23733/search-mine/issues/13) | M2 | [#12](https://github.com/crystal23733/search-mine/issues/12) | FR09 / TS17/18 | UTC/버전 재현; 정답·닉네임 없는 공유; 개인/unverified 결과 구분 |
| [#14 feat(web): 오프라인 캐시와 대기 제출 저장](https://github.com/crystal23733/search-mine/issues/14) | M2 | [#13](https://github.com/crystal23733/search-mine/issues/13) | FR10 / TS18 | shell/core/locale/daily cache; 저장소 실패 fallback; 온라인 권위 이관 금지·업데이트 경계 |
| [#15 feat(server): 4개 OAuth 인증과 최소 계정·PostgreSQL 저장 경계](https://github.com/crystal23733/search-mine/issues/15) | M3 | [#14](https://github.com/crystal23733/search-mine/issues/14) | FR01/16, NFR02 / TS01/20/29/31~36 | Google·Apple·카카오·네이버 실제 연동; provider port/adapter; 최소 scope·claim whitelist·비밀번호 없음; 거래 바인딩/JWKS·HttpOnly·CSRF; 명시적 연결/export/delete·Apple 암호화 credential/철회; sqlx migration·실DB·E2E |
| [#16 feat(server): 권위 매치 actor와 WebSocket 계약](https://github.com/crystal23733/search-mine/issues/16) | M3 | [#46](https://github.com/crystal23733/search-mine/issues/46) | FR02~05/15 / TS14/20/21/22 | 직렬입력·deadline·공개DTO; frame/rate/capacity; 결과 transaction 1회 |
| [#17 feat(server): 빠른 대전 백필과 친구 방](https://github.com/crystal23733/search-mine/issues/17) | M3 | [#16](https://github.com/crystal23733/search-mine/issues/16) | FR06/07 / TS10/12 | 10초 원자 백필·봇표시; 만료8문자 코드·2seat; cancel/ready/열거제한; OAuth 왕복 초대 코드의 서버 거래 보존·nickname/tutorial 뒤 방 복귀 |
| [#18 feat(server): 재접속·중복 명령·장애 판정](https://github.com/crystal23733/search-mine/issues/18) | M3 | [#17](https://github.com/crystal23733/search-mine/issues/17) | FR14 / TS15/16 | 30초유예·epoch·snapshot; duplicate ack; forfeit/abandon/abort 구분 |
| [#19 feat(server): 데일리 replay 검증과 순위표](https://github.com/crystal23733/search-mine/issues/19) | M3 | [#18](https://github.com/crystal23733/search-mine/issues/18) | FR09/10 / TS17/18 | 유효입력·버전 replay; 첫 완료 unique; online시간/offline완료 순위와 신뢰 표기 |
| [#20 feat(server): 자체 제품 이벤트·보존·삭제](https://github.com/crystal23733/search-mine/issues/20) | M3 | [#19](https://github.com/crystal23733/search-mine/issues/19) | FR16 / TS29 | event_id dedup·동의 purpose 분리; 원시30일/집계13개월; NSM·코호트/삭제 검증 |
| [#21 feat(web): 인증 CMP·8언어 정책·저장소 고지](https://github.com/crystal23733/search-mine/issues/21) | M4 | [#20](https://github.com/crystal23733/search-mine/issues/20) | FR13 / TS24/29 | 사용자 실제 운영자/계정정보; 거부·실패·철회 무광고; 인증CMP검수·실제ads.txt |
| [#22 feat(web): 메뉴·결과 광고와 3판 빈도 제한](https://github.com/crystal23733/search-mine/issues/22) | M4 | [#21](https://github.com/crystal23733/search-mine/issues/21) | FR12 / TS23/25 | 완료 gap≥3·1slot; 게임중 요청/표시0; timeout/no-fill·다중탭 무차단 |
| [#23 feat(web): locale SEO와 번역·접근성 최종 검수](https://github.com/crystal23733/search-mine/issues/23) | M4 | [#22](https://github.com/crystal23733/search-mine/issues/22) | FR11/13, NFR06 / TS19/26 | 8locale HTML/canonical/hreflang/sitemap; 개인정보화면 noindex; 긴문구/keyboard 수동 검수 |
| [#24 chore(deploy): 홈서버 Compose·Tunnel·운영 runbook](https://github.com/crystal23733/search-mine/issues/24) | M5 | [#23](https://github.com/crystal23733/search-mine/issues/23) | FR16, NFR07 / TS20/28 | host게임/DB포트0·내부망; secret/nonroot·health/drain; 사용자 회선/계정 확인 |
| [#25 chore(ops): PostgreSQL 백업·복구·롤백 훈련](https://github.com/crystal23733/search-mine/issues/25) | M5 | [#24](https://github.com/crystal23733/search-mine/issues/24) | FR16, NFR07 / TS28 | 별도매체 암호화 dump/checksum; 빈 DB복구·이전image호환; 실측 RPO/RTO |
| [#26 test(perf): 미니 PC 부하·번들·글로벌 지연 측정](https://github.com/crystal23733/search-mine/issues/26) | M5 | [#25](https://github.com/crystal23733/search-mine/issues/25) | NFR03/04/07 / TS21/27 | 실사양·k6 실제매치/공격; admission측정안정치70%; 초기/게임/WASM·RTT별도 |
| [#27 chore(product): 플레이테스트·공정성·튜닝 평가](https://github.com/crystal23733/search-mine/issues/27) | M6 | [#26](https://github.com/crystal23733/search-mine/issues/26) | FR15/16 / TS07/13/27 | E1/E2 모집·원자료/표본; 수치수정은설계먼저; NSM·재대결·공정성 출구평가 |
| [#28 chore(product): 28일 광고 현금 손익과 성장 평가](https://github.com/crystal23733/search-mine/issues/28) | M6 | [#27](https://github.com/crystal23733/search-mine/issues/27) | FR12/16 / TS23/29 | 전력·도메인·정산실측; 동의/광고코호트 편향; E5·확장여부 사용자 결정 |
| [#42 chore(auth): 실제 OAuth 등록·키·최소 권한 실계정 검수](https://github.com/crystal23733/search-mine/issues/42) | M3 | 외부 입력 | FR01/16, NFR01/02 / TS01/20/29/31~36 | 등록 도메인·네 제공자 키·최소 권한·실계정 로그인/연결/삭제 검수; 비밀값 공개 금지 |
| [#43 feat(auth): 최소 계정·인증 거래·세션·PostgreSQL 기반](https://github.com/crystal23733/search-mine/issues/43) | M3 | [#14](https://github.com/crystal23733/search-mine/issues/14) | FR01/16, NFR01/02 / TS01/20/29/31~36 | 최소 도메인·HMAC/AEAD·5분 거래 원자 소비·세션 회전/철회·별도 migration·실DB 검증 |
| [#44 feat(auth): 네 OAuth 어댑터와 인증 HTTP 경계](https://github.com/crystal23733/search-mine/issues/44) | M3 | [#43](https://github.com/crystal23733/search-mine/issues/43) | FR01/16, NFR01/02 / TS01/20/29/31~36 | 네 최소 제공자·JWT/JWKS·bootstrap/start/callback/session·Origin/CSRF·HTTP 계약 |
| [#45 feat(auth): 명시적 연결·계정 권리·Apple 철회](https://github.com/crystal23733/search-mine/issues/45) | M3 | [#44](https://github.com/crystal23733/search-mine/issues/44) | FR01/16, NFR01/02 / TS01/20/29/31~36 | 최근 재인증·연결/충돌/마지막 수단·export/delete·Apple 암호 credential/알림/철회 |
| [#46 feat(web): 네 OAuth 로그인·최소 계정 관리 화면](https://github.com/crystal23733/search-mine/issues/46) | M3 | [#45](https://github.com/crystal23733/search-mine/issues/45) | FR01/16, NFR01/02 / TS01/20/29/31~36 | 8언어 Atomic 로그인/계정·무계정 유지·secret 저장 금지·세션 revision·PC/mobile E2E |

#15는 상위 출시 검수이며 #43→44→45→46을 순차 병합한다. #42 실제 검수 전 상위 이슈는 열어둔다. #16 구현 선행은 #46이다. [ADR0019](../adr/0019-auth-foundation-and-delivery.md).

사용자가 전체 작업의 리뷰·검사·병합·순차 진행을 위임했다. M0 #29→#30 병합 후 #4부터 TDD 구현한다. 외부 키가 없는 #15 실계정 검수는 후속 사용자 입력 이슈에 남기고 계약 테스트를 포함한 구현과 이후 작업을 계속한다. 실제 인증/검수 완료를 가장하지 않는다.

백로그 데이터의 원천은 [backlog.json](backlog.json)이다. 변경 때 GitHub 이슈 제목·인수 조건과 함께 갱신한다.

#72/PR73은 develop1b619ec2에 병합·종료했고 상위17의 통합 출구도 완료·종료했다. CI18 DB26·138unit/75browser·6checks를 통과했다. 다음 [#74 재접속 입력 순번 복구](https://github.com/crystal23733/search-mine/issues/74)는 [ADR0035](../adr/0035-reconnect-command-cursor.md)의 본인 cursor·새 epoch cached ACK와 실제 WS/HTTPS reload를 연결한다. 자동 backoff·grace 공개 상태/재전송·영속 서버 재시작 복구는 상위18의 후속 하위 작업이다.

#70은 PR71/develop37914fd5에 병합·종료했다. CI18 DB26·웹128unit/72browser와6checks를 통과했다. [#72 친구 방·초대·준비·온라인 화면](https://github.com/crystal23733/search-mine/issues/72)은 [ADR0034](../adr/0034-friends-room-ui.md)의8문자/명시 참가·현재 방/ready/취소·공유·8언어와 실제 SQL/HTTPS/WS 출구를 연결했다.

#68은 PR69/develop4686ceb4에 병합·종료했다. [#70 빠른 대전과 실제 온라인 보드·결과](https://github.com/crystal23733/search-mine/issues/70)는 [ADR0033](../adr/0033-online-quick-match-ui.md)의 세션 HTTP/WS·닫힌 공개 DTO·3난이도/취소·8언어·실제 SQL/WS/HTTPS를 연결했다.

#66은 PR67/developf5f9dd26에 병합·종료했다. 다음 [#68 세션 인증 요청 포트](https://github.com/crystal23733/search-mine/issues/68)는 전/후 bootstrap·fresh CSRF와 account/session/generation을 연결하고 안정된 조회의 working/revision을 보존한다. [ADR0032](../adr/0032-session-bound-web-requests.md). 로비/온라인 화면·재접속과 상위17의 통합 출구는 후속이다.

#16은 PR51/develop951dff11에 병합·종료했다. 상위 #17은 [#52 원자 큐·방 정책](https://github.com/crystal23733/search-mine/issues/52)부터 순차 검토·병합하며, 이후 비공개 보드/봇 실행과 인증 전송/OAuth 초대·화면의 별도 하위 작업을 연결한다. #17의 통합 출구는 모든 하위 작업이 완료될 때 닫는다. [ADR0024](../adr/0024-atomic-lobby-and-online-composition.md).

#52는 PR53/develop16b7495d에 병합·종료했다. 다음 [#54 검증된 비공개 보드 공급](https://github.com/crystal23733/search-mine/issues/54)은 source port/풀을 구현하며 실제 매치 조립·온라인 봇·인증 전송/초대/화면은 후속 #17 하위 작업이다. [ADR0025](../adr/0025-bounded-private-certified-board-pool.md).

#54는 PR55/develop8d2d08e에 병합·종료했다. 다음 [#56 공개 관측 온라인 봇 actor](https://github.com/crystal23733/search-mine/issues/56)은 독립 행동 RNG·bounded blocking·기존 serial actor를 연결한다. [ADR0026](../adr/0026-public-observation-online-bot-driver.md). 실제 reservation→registry 조립·인증 전송/초대/화면은 다음 하위 작업이며 상위17은 OPEN이다.

#56은 PR57/develop8dc226b에 병합·종료했다. 다음 [#58 실제 매치 조립](https://github.com/crystal23733/search-mine/issues/58)은 core prepare/start와 bounded LobbyService를 연결한다. [ADR0027](../adr/0027-atomic-lobby-match-composition.md). 인증 전송·OAuth invite·화면/main은 후속이며 상위17은 OPEN이다.

#58은 PR59/develop0a9eb49에 병합·종료했다. 다음 [#60 세션 수명과 원자 로비 입장](https://github.com/crystal23733/search-mine/issues/60)은 별도 공유 lease·합성 철회 barrier·준비 완료 시 두 사람의 권한을 연결한다. [ADR0028](../adr/0028-session-bound-lobby-admission.md). 최초 WS 연결 수명·공개 HTTP/main·OAuth invite·8언어 화면은 이어지는 하위 작업이며 상위17은 OPEN이다.

#60은 PR61/develop904de85에 병합·종료했다. 다음 [#62 배정 후 최초 게임 연결](https://github.com/crystal23733/search-mine/issues/62)은 초기 lease를 actor로 넘기고 시작 기한의 미연결/철회를 취소한다. [ADR0029](../adr/0029-initial-match-connection-lifecycle.md). 공개 HTTP/main·OAuth invite·8언어 화면의 실제 통합 출구는 후속이며 상위17은 OPEN이다.

#62는 PR63/developba91e66에 병합·종료했다. 다음 [#64 인증 로비 HTTP와 실행 구성](https://github.com/crystal23733/search-mine/issues/64)은 Rust→TS·Origin/CSRF·bounded read/rate·persistent 철회와 main을 연결한다. [ADR0030](../adr/0030-authenticated-lobby-http-runtime.md). OAuth 초대·8언어 로비/온라인 화면은 후속이며 상위17은 OPEN이다.

#74는 PR75/develope27f9226에 병합·종료했다. CI 실제 PG18.6 DB26·웹141unit/77browser·최신6checks가 통과했다. 이전 offline-mobile 두 탭 update 실패는 원인 미확정으로 PR75에 보존했고 테스트 진단을 추가했다. 다음 [#76 상대 연결 유예와 화면](https://github.com/crystal23733/search-mine/issues/76)은 [ADR0036](../adr/0036-opponent-reconnect-grace.md)의 Rust 공개 상태와8언어 표시를 연결한다. 자동 backoff/미확인 명령·영속 재시작/결과 재조회와 상위18은 후속이다.

#76/PR77은 developc25181e3에 병합·종료했다. CI PG18.6 DB26·143unit/79browser(재시도0)·최신6checks를 통과했다. 다음 [#78 비권위 세션 복구 후보](https://github.com/crystal23733/search-mine/issues/78)는 [ADR0037](../adr/0037-session-recovery-candidate.md)의 권한 차단/메모리 후보/동일 세션 fresh proof·단조 기한을 연결한다. 자동 WS backoff/명령 재전송·영속 재시작/결과 조회와 상위18은 후속이다.

#80/PR81은 developc0fa4875에 병합·종료했다. CI PG18.6 DB26·176unit·browser84pass+기존offline1flaky와 최신6checks SUCCESS를 확인했다. [#85 업데이트 진단 보존](https://github.com/crystal23733/search-mine/issues/85) → [#84 로컬 단위 테스트 준비/DOM 조회](https://github.com/crystal23733/search-mine/issues/84) → [#82 인증된 개인 결과 조회](https://github.com/crystal23733/search-mine/issues/82) 순으로 진행한다. 원인 미확정 #83은 OPEN이며 #85의 CI 자료로 분석한다. #84/#82는 원인 분석과 독립적이다. timeout/retry를 완화하지 않는다. 상위18과 영속 재시작 출구는 아직 OPEN이다.

#85/PR86은 developa7c3099에 병합·종료했다. 최종 CI PG18.6 DB26·176unit/85browser 재시도0·최신6checks 통과와 실패/성공 artifact의 실제 보존을 확인했다. #84는 [테스트 설계](../design/14-testing-strategy.md)의 순수 port Node/DOM jsdom 분리·독립 행동/정상 poll 관측으로 진행한다. 로컬 실제 결과와 미확인 환경 원인은 [검증84](../verification/84-unit-arrangement-observation.md)에 구분한다. #82는 #84 병합·종료 후 시작하며 #83은 열어 둔다.

#84/PR87은 develop2db4ac7에 병합·종료했다. 최종CI181unit15.37초/line87.52%·85browser4.9분/재시도0·PG18.6 DB26·최신6checks를 확인했다. #82는 [ADR0039](../adr/0039-authenticated-personal-result-read.md)의 actor 독립 본인 최소 결과·저장 hash·90일 조회·별도 철회 권위·단일2초/동시/rate를 구현한다. 실행 근거는 [검증82](../verification/82-authenticated-personal-result-read.md)에 기록한다. 웹 소비/영속 journal/재시작은 후속18이며 #83 원인은 미확정이다.

#82/PR88은 develop cdd700e에 병합·종료했다. CI PG18.6 DB30·181unit/85browser 재시도0·Result100%/전체92.77%를 확인했다. 다음 [#89 본인 저장 결과 웹 확인](https://github.com/crystal23733/search-mine/issues/89)은 [ADR0040](../adr/0040-personal-result-web.md)의 현재 메모리 match/strict 최소 DTO·전후 인증·취소/WS 우선·8언어와 actual PG/HTTPS를 구현한다. 영속 journal/재시작 복구18과 원인 미확정83은 별도다.

#91 auth-recovery-observation: 선행89/PR90병합·종료 후 실제HTTPS 철회 응답의 관측 수명을 안정화한다. [ADR0041](../adr/0041-auth-recovery-response-observation.md)/[검증91](../verification/91-auth-recovery-observation.md). 미확인 통계/journal/startup abort는 후속18,83원인미확정OPEN.

#91/PR92은 develop8347efa에 병합·종료했다. 다음 [#93 미확인 통계 계약](https://github.com/crystal23733/search-mine/issues/93)은 [ADR0042](../adr/0042-unknown-result-details.md)의 명시 null·DB/HTTP/8언어 화면을 연결한다. 실제 영속 journal/startup abort·새로고침 발견과 상위18은 후속이며 #83/42/26/27 출구는 별도다.

#93/PR94은 develope5493be에 병합·종료했다. CI89passed+1flaky의 기존 known 결과 본문 관측은 다음 [#95](https://github.com/crystal23733/search-mine/issues/95)가 [ADR0043](../adr/0043-result-response-observation.md)에 따라 수정한다. 제품 결과 계약은 통과했으며 실제 영속 소유 연결/journal/startup/processkill·새로고침 발견과 상위18은 후속이다.

#95/PR96은 developfd29c11에 병합·종료했다. 다음 [#97](https://github.com/crystal23733/search-mine/issues/97)은 [ADR0044](../adr/0044-online-storage-owner.md)의 단일 저장 소유 연결을 구현한다. active journal/admission·startup abort/OS kill-restart·새로고침 발견은 후속 #18이다.

#97/PR98은 developecb6f21에 병합·종료했다. 다음 [#99](https://github.com/crystal23733/search-mine/issues/99)는 [ADR0045](../adr/0045-result-retention-anchor.md)의 보존 기준을 분리한다. journal/admission/startup abort·실제 OS kill-restart 결과 복구·새로고침 발견은 후속 #18이다.
