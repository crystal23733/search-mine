# ADR0058: 최소 정보의 공개 데일리 순위 조회

- 상태: 승인된 FR09의 상세 구현 결정
- 이슈: [#124](https://github.com/crystal23733/search-mine/issues/124), M3/상위 #19
- 선행: #122/PR123 develop1414cda 병합·#122 CLOSED 실제 확인
- 추적: FR01/09/10/16, NFR01/02/05/07/08, TS17/18/20/21/31/35/36

## 결정

제공 디자인11의 완료 시간 순서와 기존 프로토콜의 완료/오류 순위를 분리한다. 시간 순위는 저장된 verified_elapsed_ms가 있는 최초 완료만 (서버 시간, 오류, completed_at, 내부 completion UUID) 오름차순으로 조회한다. 완료 순위는 NULL 시간을 포함한 모든 검증 완료를 (오류, completed_at, 내부 completion UUID) 오름차순으로 조회한다. client replay 시간은 정렬하거나 응답하지 않는다. position은 이 안정적 정렬에서의 1부터 시작하는 위치이며 동점 공동 등수가 아니다.

고정 POST /api/v1/daily/rankings는 {v,date,seed_version}만 받는다. 로그인·쿠키·CSRF·계정 생성 없이 같은 origin의 JSON 읽기를 제공한다. 외부 origin/누락·중복 Origin, query, 알 수 없는 필드·버전·날짜, 입력4KiB 초과를 거절한다. 응답은 v, accepted metadata(nullable), time/completion의 entries와 has_more다. 각 목록은 최대50개이며 51번째 존재만 표시한다. 날짜/버전에 아직 publication이 없으면 metadata:null/빈 목록이다. 조회가 definition을 생성하지 않는다.

각 항목에는 position·현재 게임용 nickname·mistakes만, 시간 목록에는 검증된 elapsed_ms도 포함한다. 타인의 계정/attempt/completion ID·completed_at·digest·replay·subject·token과 client elapsed는 공개하지 않는다. 내 개인 기록은 기존 인증 own API를 쓴다. 전체 COUNT/임의 offset/50위 밖 개인 등수는 제공하지 않으며 UI는 상한을 안내한다.

## 경계와 저장소

별도 DailyRankingReader port와 serializable하지 않는 내부 snapshot/row를 사용한다. PG adapter는 accepted definition과 두 목록을 같은 repeatable-read/read-only transaction에서 읽고 현재 auth_accounts nickname을 JOIN한다. 삭제는 후속 조회에서 반영되며 이미 읽힌 응답을 소급 회수한다고 주장하지 않는다. 닉네임은 기존 Nickname parser와 canonical 값으로 검증한다. 기존 publication/receipt 검증을 재사용하고 손상·clock 역행은 빈 성공으로 숨기지 않는다. 정확90일 record 만료는 제외한다. 기존 migration checksum은 변경하지 않고 두 정렬을 지원하는 forward index를 추가한다. SQL LIMIT51, query bound/DB2초와 응답64KiB를 유지한다.

공개 읽기는 별도의 requests2와 global20초당 제한을 둔다. IP/브라우저/계정별 추적 없이 기존 ResultRate에 단일 고정 budget key를 사용한다. 인증 데일리와 공개 읽기는 같은 BoundedReplayVerifier clone의 CPU2/2초 예산을 공유한다. 전체 HTTP는2초이며 body extraction 이전의 네트워크 전송 시간을 이 기한으로 주장하지 않는다. no-store/no-referrer/nosniff와 비활성503 경로를 제공한다. 실제 main과 HTTPS fixture 모두 같은 구성이다.

## 검증과 후속

공개 HTTP가 기존404인 Red를 실제 실행한 뒤 구현한다. 순수 projection에서 닉네임·secret 제외·accepted 정의·타입/순서/중복/상한/만료/시간 trust를 검사한다. 실PG에서 시간/NULL의 분리·동점·51건 상한·현재 nickname/삭제·보존/손상·readonly/timeout을 확인한다. HTTP의 origin/JSON/closed/상한/rate/동시성/clock/기한·공유 CPU와 실제 HTTPS PC/mobile도 검사한다. full·도메인95/전체80·문서/Mermaid·코드 리뷰·최신CI·정확head 병합/종료를 확인한다. 공개 replay 일관성은 인간 풀이/사전 계산 방지/RTT 공정성 증거가 아니다.

8언어 공식 도전·heartbeat/대기 제출 UI와 상위19 통합은 다음 순차 작업이다. 외부42·물리25·하드웨어26·사람27·미확정83은 미완료다.
