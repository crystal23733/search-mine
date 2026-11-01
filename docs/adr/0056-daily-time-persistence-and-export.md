# ADR0056: 본인 내보내기와 검증된 데일리 시간 영속화

> 2026-11-01 대체 범위: [ADR0081](0081-my-info-and-export-removal.md)의 사용자 지시로 파일 다운로드/공개 export·raw replay 제공과 동의 내보내기 후속 계획을 내 정보 화면의 조회·관리로 전환한다. 이 문서의 당시 구현·실행 근거는 보존하며 소유권·재인증·철회·삭제·보존 원칙은 유지한다. 실제 코드 제거 완료는 #178에서 확인한다.

- 상태: 채택 (승인된 #19/#118의 순차 구현)
- 일자: 2026-10-12
- 연결: #120, 선행 #118/PR119 develop307a58d 병합·CLOSED 확인
- 요구: FR01/09/10/16, NFR01/02/05/07/08, TS17/18/20/21/31/35/36

## 문제와 결정

[ADR0055](0055-bounded-daily-time-authority.md)의 sealed VerifiedDailyTime은 메모리에만 존재한다. 시간을 수집하기 전에 본인 내보내기에 같은 값을 제공해야 한다. 먼저 Rust ExportRecord::Daily와 DailyCompletion에 필수 nullable `verified_elapsed_ms: Option<u32>`를 추가하고 생성 TS·strict 웹 decoder·설정 다운로드를 준비한다. 이어 forward migration과 실제 PostgreSQL 저장/읽기를 구현한다. 기존 응답의 필드 누락을 null로 추정하지 않는다. 현재 고정 submit은 completion-only로 유지한다. 공개 start/heartbeat/timed-submit·순위·명시 데일리 UI 활성화는 다음 이슈다.

## 봉인·첫 기록·신뢰

DailyRepository에는 sealed time을 받는 별도 submit_timed 포트를 추가한다. 기본 구현은 Unavailable이며 기존 어댑터가 유효 시간 proof를 조용히 버리지 못한다. PgDailyRepository는 plain/timed 모두 같은 bounded transaction 구현을 사용한다. 새 time의 account/attempt/accepted metadata/canonical digest는 실제 ReplayEvidence와 `matches`로 결합하며 불일치는 Unavailable/쓰기0이다. private fields/비직렬화 proof만 받고 클라이언트 elapsed/final tick을 공식값으로 복사하지 않는다. 실제 계정/session transaction·게시 snapshot 검증·clock 재관측·닉네임·삭제 직렬화는 그대로 적용한다.

첫 row의 nullable 시간을 불변 저장한다. 원래 attempt의 동일 replay retry는 ticket 소실/재시작 또는 새 proof의 다른 elapsed와 관계없이 저장된 원래 receipt를 반환한다. 처음 NULL로 완료한 기록은 나중에 time으로 승격하지 않는다. 다른 attempt는 Practice로 최초 receipt를 반환하며 덮어쓰지 않는다. metadata/digest가 달라진 동일 attempt는 Conflict다. HTTP trust는 검증된 저장 receipt의 Some에만 verified_time, None/own 없음은 verified_completion이다. 두 순위가 구현되었다는 뜻은 아니다.

## DB·검증·권리

기존 migration202610110001 checksum을 보존하고 새 nullable bigint 열과 0..u32::MAX CHECK를 forward migration으로 추가한다. 기존 행은 NULL이다. 기존 to_jsonb 기반 protect_daily_completion은 새 열도 UPDATE 불변으로 보호하며 실제 DB에서 확인한다. shared receipt 검증은 Some >= native semantic replayed_elapsed_ms 및 Some < accepted rules.duration_ms를 요구한다. TTL의 정확 만료 경계에서는 proof를 만들 수 없다. 손상/overflow/짧은 시간/상한은 Unavailable로 실패하며 NULL로 숨기지 않는다. generic export port/web은 u32와 >= semantic elapsed를 검사하고 accepted duration은 DB의 shared validator가 담당한다. TS에 게임 규칙을 복제하지 않는다.

본인 export의 SQL·DTO·strict 다운로드는 시간을 누락 없이 제공한다. 50개 페이지/32MiB·최근 인증·현재 session revision/철회·raw7일/record90일·account cascade 삭제를 유지한다. 새로운 식별자/PII/raw cache를 만들지 않는다. migration → 같은 계약의 server/web 순서로 운영하되 이 단계에서 배포하지 않는다.

## 검증과 제한

필수 nullable field의 실제 Red→Green을 먼저 확인한다. 실제 native evidence와 실제 ticket registry의 주입 clock/유효 authority로 만든 sealed proof를 실제 PG에 저장한다. 결합 불일치·동시 plain/timed first commit·원래 retry/Practice/NULL 불변·DB CHECK/UPDATE·손상 fail closed·권한/삭제/보존·본인 HTTP/HTTPS 다운로드를 검사한다. 회귀/full/fresh domain95/전체80·문서/Mermaid·코드리뷰·최신CI/exact-head 병합/120CLOSED를 완료한다. fake clock 기반 성공을 실제 벽시계/인간 풀이/사전 계산 방지/글로벌 RTT 공정성으로 주장하지 않는다. 상위19·공개 시간 HTTP/두 순위/UI와 외부42·물리25·하드웨어26·사람27·SW83은 미완료다.
