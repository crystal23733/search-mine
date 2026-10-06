# 20. 이벤트와 대시보드

> 적용: pm-product-discovery:metrics-dashboard · 입력: [NSM](09-north-star.md), [시나리오](19-test-scenarios.md)

## 이벤트 계약

공통 필드는 event_id(UUID), event_version, occurred_at(UTC), received_at, guest_id(동의된 제품 분석 또는 필수 서버 기록 구분), session_id, locale, rules_version, source(server/client), mode다. 닉네임·원시 IP·토큰·시드·채팅·정답을 이벤트에 넣지 않는다. event_id unique로 중복 제거한다.

| 이벤트 | 생성 주체 / 필드 | 신뢰 |
|---|---|---|
| landing_view, invite_open | 클라이언트 / route, invite 여부 | 비필수 분석 동의 후 |
| tutorial_start, tutorial_complete | 클라이언트 / duration_ms, skipped | 비필수 분석 동의 후 |
| match_queued, match_start, match_end | 서버 / match_id, opponent_type, end_reason, completed | 운영·판정 필수 기록 |
| lie_applied, accusation_result | 서버 / match_id, success, proof_version | 위치·정답 제외, 집계 |
| rematch_selected | 클라이언트 / previous_match_id | 분석 동의 후 |
| daily_verified | 서버 / daily_id, errors, verification_status | 순위 필수 기록 |
| share_click | 클라이언트 / daily_id, channel | 분석 동의 후, 실제 공유 성공과 구분 |
| consent_changed | 클라이언트 / policy_version, 상태 enum | 동의 상태 최소 기록 |
| ad_request, ad_impression, ad_failed | 클라이언트 / placement, provider enum | 광고 동의 후, 실제 SDK callback만 |

## 정의와 경보

| 지표 | 분자 / 분모 / 창 | 출처 / 시각화 | 목표 / 조사 기준 |
|---|---|---|---|
| NSM | UTC주 정상 완료≥2 guest 수, 튜토리얼·abort 제외 | match_end / 주 추세 | 베이스라인 뒤 목표, 2주 연속 20% 감소 조사 |
| 학습 완료율 | 완료 session / 시작 session, 24h 내 | tutorial / funnel | ≥80%, <60% 조사; skip 별도 |
| 첫 판 완료율 | 정상 완료 guest / 첫 match_start guest, 24h | server / 비율 | 베타 기준선 설정 |
| 재대결률 | 결과 10분 이내 다음 시작 guest / 정상 첫 결과 guest | server / 비율 | E2≥60%, <40% 재설계 |
| 초대 전환 | invite_open 후 10분 내 친구 시작 session / invite_open session | client+server / funnel | 베이스라인 뒤 목표 |
| 공유 클릭률 | share_click session / daily 완료 session | 동의 코호트 / 비율 | ≥5%, <2% 문구 조사 |
| D7 | 첫 정상 판 날짜+7 UTC일 정상 판 guest / 해당 첫 판 guest | server / cohort | 표본≥100 뒤 목표 |
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
