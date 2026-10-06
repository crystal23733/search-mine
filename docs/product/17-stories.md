# 17. 사용자·Job Stories

> 적용: pm-execution:user-stories + job-stories · 입력: [PRD](16-PRD.md)
> Card=각 행의 이야기, Conversation=설계 링크의 경계·예외, Confirmation=인수 조건. INVEST 기준으로 이슈를 나누며 실제 의존성은 로드맵에 명시한다.

| ID | 사용자 이야기 / 가치 | 설계 | Confirmation |
|---|---|---|---|
| US01 | 방문자로서 계정 없이 시작해 준비를 줄인다 | [보안](../design/12-security.md) | 세션 생성; 닉네임 검증; 쿠키 HttpOnly; 삭제/만료 |
| US02 | 플레이어로서 같은 논리 판에서 실력을 겨룬다 | [알고리즘](../design/05-algorithms.md) | 동일 오프닝; 솔버 증거; native/WASM 일치; 비밀 미노출 |
| US03 | 플레이어로서 실수 후에도 승부를 계속한다 | [규칙](../design/04-game-rules-spec.md) | 3초 입력 차단; 자동 지뢰 표시; 진행률 분모 일정; 종료 후 입력 거절 |
| US04 | 공격자로서 자원을 써 상대 숫자를 속인다 | [알고리즘](../design/05-algorithms.md) | 닫힌 비제로; ±1 범위; 최대2; 증명 실패시 미차감 |
| US05 | 방어자로서 논리로 지목해 반격한다 | [규칙](../design/04-game-rules-spec.md) | 원래 숫자 복원; 공격자2초; 게이지0; 오지목3초 |
| US06 | 대기자로서 10초 뒤 표시된 봇과 시작한다 | [프로토콜](../design/06-protocol.md) | 대기 deadline; 인간/봇 원자 선택; 난이도3; 봇 정보 제한 |
| US07 | 친구로서 코드로 같은 방에 들어간다 | [프로토콜](../design/06-protocol.md) | 2자리 예약; 만료/틀린 코드 오류; 중복 거절; 시작 확인 |
| US08 | 초보자로서 공격과 지목을 직접 배우고 싶다 | [UX](../design/09-screens-ux.md) | 첫 방문 자동; 각1회; 반복/건너뛰기; 초대 정보 유지 |
| US09 | 솔로로서 오늘의 기록을 안전하게 공유한다 | [DB](../design/07-database.md) | UTC 공통 판; replay 검증; 첫 완료 한 건; 정답 없는 카드 |
| US10 | 끊김 중에도 캐시된 판으로 연습한다 | [클라이언트](../design/08-client-architecture.md) | 캐시 확인; unverified 표기; idempotent 제출; 온라인 승부 이관 금지 |
| US11 | 글로벌 사용자로서 이해하는 언어로 플레이한다 | [i18n](../design/10-i18n.md) | 8언어; 감지/설정; en fallback; URL·hreflang·상태 유지 |
| US12 | 무료 사용자로서 판이 끝난 뒤에만 광고를 본다 | [광고](../design/11-ads-consent.md) | 3판 cap; 플레이 노출0; 차단/실패 무차단; SDK 중복 방지 |
| US13 | 사용자로서 저장소와 광고 동의를 통제한다 | [광고](../design/11-ads-consent.md) | 거부 가능; CMP 실패 광고차단; 철회; 8언어 정책·ads.txt |
| US14 | 대전자로서 재연결 후 같은 상태로 복귀한다 | [프로토콜](../design/06-protocol.md) | 30초 유예; revision 복구; 입력 dedup; 만료/장애 판정 |
| US15 | 운영자로서 같은 설정을 재현한다 | [규칙](../design/04-game-rules-spec.md) | 버전/hash; 시작 snapshot; 중도변경 금지; 단일 원천 |
| US16 | 운영자로서 백업과 지표로 안정성을 관리한다 | [운영](../design/16-observability-ops.md) | 내부 DB; restore 성공; event dedup; 보존/삭제 |

## Job Stories

### JS01: 낯선 숫자가 모순일 때 확신을 되찾는다

When 열린 숫자가 다른 정보와 맞지 않을 때, I want to 안전한 근거로 지목하고, so I can 공격을 반사하고 승부를 계속한다.
인수 조건: 열린 숫자만 지목; 키보드/터치 제공; 추측 없는 식별 증거; 올바른 지목 숫자 복원; 반사 상태 피드백; 오지목 3초; 재지목 중복 거절; [규칙](../design/04-game-rules-spec.md)과 일치.

### JS02: 회선이 끊겼을 때 결과를 잃지 않는다

When 홈서버 접속이 안 될 때, I want to 캐시된 데일리를 연습하고 결과를 보관해, so I can 복구 후 검증을 요청한다.
인수 조건: 오프라인 표시; 캐시 없을 때 안내; 날짜/버전 고정; 로컬 순위 구분; 입력 로그 저장; 복구 시 중복 제출 방지; 만료 시 개인 기록 유지; 온라인 대전 결과 조작 금지.

### JS03: 광고가 불편할 때 게임을 계속한다

When 광고에 동의하지 않거나 SDK가 실패할 때, I want to 다음 판으로 바로 이동해, so I can 무료 게임 가치를 유지한다.
인수 조건: 거부 경로; 필수 저장소 설명; 비필수 태그 차단; CMP 실패 무광고; SDK 대기 timeout; 배너 영역 안정성; 재대결 지연 없음; 철회 후 추가 요청 차단.

## 결정 사항

이야기의 인수 조건을 [테스트 시나리오](19-test-scenarios.md)로 옮긴 뒤 구현한다.

## 열린 질문

온라인·오프라인의 기록 표시 문구와 글로벌 닉네임 필터 정책을 검수한다.
