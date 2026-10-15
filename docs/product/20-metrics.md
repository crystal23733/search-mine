# 20. 이벤트와 대시보드

> 적용: pm-product-discovery:metrics-dashboard · 입력: [NSM](09-north-star.md), [시나리오](19-test-scenarios.md)

## 이벤트 계약

이벤트의 최소 공통 필드는 event_id(UUID), event_version, occurred_at(UTC), source(server/client)다. 저장 시각 received_at은 저장 adapter가 소유하고, account_id는 필수 서버 기록에만, session_key는 비필수 client의 일시적 탭 상관에만 사용한다. auth session ID/토큰을 복사하지 않는다. locale·rules_version·mode는 해당 이벤트에 필요한 경우에만 닫힌 계약으로 추가한다. 닉네임·OAuth subject·이메일·실명·사진·전화·원시 IP·토큰·시드·채팅·정답을 이벤트에 넣지 않는다. event_id unique로 중복 제거한다. 아래 표는 전체 목표이며 #134가 구현한 부분은 ADR0063의 client9종/server2종 순수 계약이다. 실제 수집·저장·운영 집계는 후속 #20에서 연결한다.

| 이벤트 | 생성 주체 / 필드 | 신뢰 |
|---|---|---|
| landing_view, invite_open | 클라이언트 / route, invite 여부 | 비필수 분석 동의 후 |
| tutorial_start, tutorial_complete | 클라이언트 / duration_ms, skipped | 비필수 분석 동의 후 |
| match_queued, match_start, match_end | 서버 / match_id, opponent_type, end_reason, completed | 운영·판정 필수 기록 |
| lie_applied, accusation_result | 서버 / match_id, success, proof_version | 위치·정답 제외, 집계 |
| rematch_selected | 클라이언트 / previous_match_id | 분석 동의 후 |
| daily_verified | 서버 / daily_id, errors, verification_status | 순위 필수 기록 |
| share_click | 클라이언트 / daily_id, channel | 분석 동의 후, 실제 공유 성공과 구분 |
| consent_changed | 검증된 동의 adapter / policy_version, 상태 enum | 별도 현재 동의 권위; 불신 client 이벤트가 grant를 만들지 않음 |
| ad_request, ad_impression, ad_failed | 클라이언트 / placement, provider enum | 광고 동의 후, 실제 SDK callback만 |

## 정의와 경보

| 지표 | 분자 / 분모 / 창 | 출처 / 시각화 | 목표 / 조사 기준 |
|---|---|---|---|
| NSM | UTC주 정상 완료≥2 account 수, 튜토리얼·abort 제외 | match_end / 주 추세 | 베이스라인 뒤 목표, 2주 연속 20% 감소 조사 |
| 학습 완료율 | 완료 session / 시작 session, 24h 내 | tutorial / funnel | ≥80%, <60% 조사; skip 별도 |
| 첫 판 완료율 | 정상 완료 account / 첫 match_start account, 24h | server / 비율 | 베타 기준선 설정 |
| 재대결률 | 결과 10분 이내 다음 시작 account / 정상 첫 결과 account | server / 비율 | E2≥60%, <40% 재설계 |
| 초대 전환 | invite_open 후 10분 내 친구 시작 session / invite_open session | client+server / funnel | 베이스라인 뒤 목표 |
| 공유 클릭률 | share_click session / daily 완료 session | 동의 코호트 / 비율 | ≥5%, <2% 문구 조사 |
| D7 | 첫 정상 판 날짜+7 UTC일 정상 판 account / 해당 첫 판 account | server / cohort | 표본≥100 뒤 목표 |
| 입력 처리 | enqueue→상태 commit ms p95 / 5분 | 운영 histogram | ≤20ms, 15분 초과 조사 |
| abort율 | 서버 abort / 시작 매치, 24h | server / 비율 | <1% 목표, ≥1% 조사 |
| 현금 손익 | 실제 28일 광고 정산−전력·도메인·여유 | 월 수동 정산 / 숫자 | ≥0 목표, 음수면 확장 동결 |

NSM과 운영 통계는 필수 게임 기록에서 산출한다. 비필수 행동 분석은 동의 코호트만 계산하고 전체 사용자로 일반화하지 않는다. 봇/사람, 언어, 온라인/로컬의 분해와 표본 수를 같이 표시한다.

```text
[NSM 주 추세 / 비교주 / 표본]
[학습 funnel] [첫 판·재대결] [초대·공유]
[D7 cohort]  [공정성 반례] [abort·지연]
[28일 순수익] [현금비] [전력·운영시간]
```

운영은 매일, 제품은 매주, 현금 손익은 월간 검토한다. 운영자는 로컬 대시보드/로그로 확인하며 외부 분석 SaaS와 자동 메시지 전송은 추가하지 않는다. 원시 이벤트 30일, 익명 일별 집계 13개월 제안; [DB 보존](../design/07-database.md)을 따른다.

## 결정 사항

불신 가능한 client 이벤트로 대전 승패·정산을 확정하지 않는다. 광고 요청·노출·정산을 구분한다.

## 열린 질문

필수 처리와 비필수 분석 구분, 보존 기간의 정책 문구는 공개 전 최종 검토한다.


#134 FR16·NFR01/02/05/07/08·TS20/21/22/24/29/31/34/35/36 → [ADR0063](../adr/0063-event-domain-contracts.md) → closed client/server 권위·목적별 consent/identity·30일/달력13개월·bounded NSM/D7/완결성·domain95 누락 방지 → [검증134](../verification/134-event-domain-contracts.md). 실제 PG 수집/retention·권리/운영·상위20은 후속이다.


#136 FR01/16·NFR01/02/05/07/08·TS20/21/29/31/34/35/36 → [ADR0064](../adr/0064-essential-event-store-and-rights.md) → committed PG source/동일account 잠금·불변 dedup/원래30일·default-deny optional·본인 typed event export/삭제 → [검증136](../verification/136-essential-event-store-and-rights.md). 자동 수집/물리retention/집계·상위20은 후속이다.

#138 FR01/09/14/16·NFR01/02/05/07/08·TS15/16/17/18/20/21/29/31/35/36 → [ADR0065](../adr/0065-atomic-essential-event-producers.md) → 원본 transaction 안의 자동 essential 수집·UUID순 계정 잠금·공동 rollback·재전송 비복원·unknown OS 복구/본인 HTTPS 다운로드 → [검증138](../verification/138-atomic-essential-event-producers.md). 물리 보존·first-normal/NSM/D7·상위20 통합 출구는 후속이다.

#140 FR01/16·NFR01/02/05/07/08·TS20/21/29/31/35 → [ADR0066](../adr/0066-bounded-raw-event-retention.md) → 원래 occurred_at30일 물리 삭제·500개/transaction·2초/SKIP LOCKED·최소 row-lock 권한·main 별도 maintenance → [검증140](../verification/140-bounded-raw-event-retention.md). calendar13개월/first-normal/NSM·D7와상위20은후속이다.
