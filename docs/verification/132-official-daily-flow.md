# #132 공식 데일리 도전·본인 결과·검증 공유

[이슈132](https://github.com/crystal23733/search-mine/issues/132) · [ADR0062](../adr/0062-official-daily-flow.md) · M3/상위19.

선행130/PR131 develop2b0f60f 병합·130 CLOSED를 실제 확인했다. 승인된 FR09 흐름의 공식 시작·heartbeat 수명·본인 receipt와 공유를 연결한다. 별도 lazy 화면/controller가 typed DailyApi·AuthPort 전후 proof·Rust DailyCore/LocalController·records/activity/share 포트를 재사용한다. 새 규칙·의존성·migration·개인정보 저장 필드를 추가하지 않았다.

## 행동과 실제 통합

기존 production HTTPS의 `/en/daily`에서 공식 도전 진입점 부재로 Red exit1을 확인했다. 준비 timeout은 별도 보존하고 행동 실패로 세지 않았다. 구현은 계정/revision의 명시 확인→서버 accepted 날짜/전체 metadata·rules로 WASM 초기화→단일 순번 heartbeat→완료 replay의 unverified first 후보 선보존→현재 proof 제출이다. 완료는 진행 중 heartbeat를 기다리며 ack:null/실패 후 ticket=null로 제출한다. 실패/중단 시 후보를 유지하고 복구만으로 자동 제출하지 않는다. Practice는 원래 첫 receipt를 표시한다.

clock/core/API/session 단위 검사에서 중복/소실 재시도·accepted 불일치·factory 실패·무중첩/순번·진행 중 heartbeat 제출 장벽·이전 account ACK 격리·미완료·계정/연결/이탈·늦은 start/core/submit/own·save 경고·다른 첫 local 보존과 현재 attempt만 제거를 확인한다. DOM 검사는 게스트/현재 계정 확인·바뀐 확인 창 취소·즉시 unmount abort·본인 없음/실패/nullable 시간·복사/PNG/native share 무요청·private 결과 제거를 확인한다. allowlist 단위 검사는 공식 시간/완료 검증/로컬 신뢰와 PNG label, nickname/account/attempt/ticket/completion ID·seed/hash/정답/입력 제외를 확인한다.

실제 HTTPS·PostgreSQL17.4·production WASM의 PC1440/mobile390에서 각각 두 판을 공개 DOM 셀의 Enter 입력으로 완료했다. 명시 확인 전 start=0, 서버 verified_time·첫 완료/Practice의 원래 receipt 동일, 실제 단일 heartbeat ACK, 두 번째 제출503 후 같은 attempt 명시 재시도, 다른 첫 local record 보존, pending0/record1, PNG signature, 8언어 본인 기록 조회·가로 넘침 없음·조회/공유의 daily 쓰기 증가0·offline private 결과 제거를 확인했다. 초점 Green2개/1.3분/재시도0이며 제품 안내 보정 전 실행과 최종 full 실행을 구분한다.

## 관측 보정·코드 리뷰

초기 mock의 타입 추론과 실제 번역 버튼 이름을 수정했다. paused clock에서 Dialog의 paint 후 effect 미실행/worker advance의 기절 해제 전 단언, 모든 프레임을 재생하는 runFor의120초 도달, fastForward의 interval 개수 추정 오류를 로그/trace와 함께 보존했다. 입력마다 가상 시간을 건너뛴 뒤 공개 셀의 기절 해제를 관측하고, 페이지 이동 전 clock을 재개하며 실제 server trust를 검사한다. response는 실제200/no-store를 전달 전에 읽고 같은 response로 fulfill한다. 공통 timeout/retry·제품 시계·정책은 바꾸지 않았다.

correctness/changes/기준2b0f60f 순차 리뷰에서 accepted binding·Rust 단일 규칙, captured owner/revision/request 수명·전후 proof, heartbeat/완료 barrier와 늦은 이전 ACK, saveFirst/다른첫local·post-proof pending 제거, 원래 Practice/own receipt·nullable 서버 시간, 공유 allowlist와 UI activity/lazy/8언어를 양쪽 port까지 확인했다. 화면/DOM 리뷰에서 확정 결과 옆에 시간 검증 진행 중 안내가 남는 결함을 발견했다. ADR를 먼저 보강하고 실제 DOM Red1실패→receipt trust 안내 보정→관련20개 Green으로 수정했다. 추가 재현 가능한 제품 결함은 검토 범위에서 확인하지 못했다. 독립 보안감사·실OAuth·실기기·사람 관찰을 수행한 것은 아니다.

## 전체 검사와 남은 출구

최종 scripts/check.ps1 실제exit0: Rust399·웹265/44파일·줄3044/3351=90.83%·공식 controller216/216=100%·공식 page52/54=96.29%·Chromium113/5.4m/재시도0·문서182/Mermaid34 실패0. 새 격리 PostgreSQL17.4 DB liar_daily132full/기존migration/CARGO_INCREMENTAL=0이며 별도 실제 DB 회귀101개15.01초도 통과했다. 전체 stdout/stderr와 exit0을 로컬에 보존했다. 최신 head CI와 정확 head squash merge·CLOSED 결과는 PR·로컬 일별 리뷰에서 최종 확인한다. #19는 하위 병합 뒤 전체 인수 조건을 감사하며 OAuth42·운영25/26·사람27·원인 미확정83은 별도다.
