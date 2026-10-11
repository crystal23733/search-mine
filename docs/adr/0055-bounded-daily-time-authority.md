# ADR0055 · 제한된 데일리 시간 티켓·하트비트 권위

- 상태: 채택 · 2026-10-11 · M3 #118 / 상위 #19
- 선행: #116/PR117 develop c56832b 병합·#116 CLOSED 확인
- 추적: FR09/10 · NFR01/02/05/07/08 · TS17/18/20/21/31/35/36

## 범위와 신뢰

공식 시작부터 서버가 관측한 경과시간을 증명하는 Rust 메모리 기반을 만든다. `timing`은 주입된 단조 시각으로 동작하는 순수 정책, `tickets`는 bounded map·Clock port·기존 AuthorityRegistry/SessionInvalidator adapter다. 새 dependency·DB migration·공개 DTO·HTTP·main 활성화는 없다. 새 서버 시간의 수집·영속화 전에 후속 이슈에서 본인 export에 nullable 시간 필드를 먼저 추가한다. 이후 인증 API, 두 순위, 명시적8언어 화면/대기 복구를 순차 연결한다.

제공11의 공식 도전 시작/연습 분리와12의 서버 검증/시간/순위 표시를 확인했다. 이 단계는 화면을 구현하거나 기존 verified_completion을 verified_time으로 바꾸지 않는다. 공개 데일리는 seed를 미리 계산할 수 있으므로 시간 proof가 사람 풀이·봇 방지·전세계 네트워크 공정성을 증명하지 않는다. 서버 관측시간은 네트워크 지연을 포함한다.

## 고정된 attempt 정책

현재 세션의 authority lease·공개 session_revision UUID·attempt UUID·accepted PublishedDaily 전체 metadata/rules에 결합한다. account는 lease에서 얻고 내부 session hash는 기존 registry에만 둔다. ticket UUID는 서버가 만들고 비밀 credential이 아니다. 정확히 같은 attempt/metadata/lease/revision의 시작 재전송은 원래 ticket/TTL을 반환한다. 같은 attempt의 다른 metadata/revision은 conflict다. 다른 attempt는 계정의 기존 ticket을 교체한다. 계정당1개, 전체1..20000개(후속 운영 기본64)다. 만료·철회된 항목은 다음 작업 때 제거하며 memory에는 compact metadata/digest/시각만 있고 raw replay·닉네임·제공자 정보는 없다.

entry가 lease를 보존하므로 ticket과 authority의 용량이 같은 경우, 만료 entry가 남아 있으면 새 계정이 cleanup에 필요한 lease 자체를 얻지 못한다. `reap()`는 인증 lease 없이 주입된 단조 Clock으로 만료/철회 항목만 제거하는 housekeeping port다. 미래 HTTP는 새 generation 캡처/실제 세션 read/bind **전에** 이를 호출한다. 공개 시간 상태를 변경하는 start/heartbeat/finish는 여전히 실제 lease가 필요하다. reap에도 전역 clock 역행/poison fail closed와 두 잠금 밖의 retired drop을 적용한다.

시작의 단조 시각을 고정하고 TTL은 accepted rules의 countdown_ms+duration_ms다. 만료 경계 `now >= start + TTL`에서 proof를 주지 않는다. heartbeat 권장15초, 마지막 정상 heartbeat/시작에서45초 이상 지나면 영구적으로 그 ticket의 시간 자격을 잃는다. 새로운 heartbeat로 부활시키지 않는다. 첫 순번1부터 연속 증가하고 마지막 동일 순번 재전송은 원래 ack를 반환하며 last_seen/TTL을 연장하지 않는다. 과거·건너뛴·0 순번은 conflict/invalid다. finished 뒤 heartbeat는 시간 자격을 새로 만들지 않는다.

유효 native ReplayEvidence를 받은 뒤 server elapsed=`now-start-countdown`을 계산한다. countdown 전/45초 gap/TTL/elapsed가 replay의 의미상 완료 경과시간보다 짧으면 시간 proof 없이 유효 완료로 강등한다. metadata 불일치는 invalid다. 첫 finish만 account/attempt/metadata/canonical digest/elapsed의 private sealed VerifiedDailyTime을 만든다. 동일 digest 재시도는 원래 proof를 반환하고 경과시간을 개선·연장하지 않는다. 다른 digest는 conflict다. TTL 뒤에는 메모리 proof가 사라지며 후속PG의 기존 첫 receipt는 그대로 반환해야 한다. 클라이언트 final tick/elapsed 값을 공식 시간으로 복사하지 않는다.

단조 clock 역행은 registry를 fail closed로 고정하고 모든 ticket을 지운다. 덧셈 overflow/poisoned lock은 unavailable이며 잘못된 시간을 만들지 않는다. 재시작은 빈 registry이므로 이전 ticket의 finish는 정상 no-proof다. 미래 인증 HTTP는 현재 UTC 날짜/actual session proof·CSRF와 accepted snapshot을 확인하고 이 registry의 authority와 같은 lease를 사용해야 한다.

순수 timing 정책도 역행 관측 시 cached finished proof를 즉시 버린다. adapter의 전역 fail closed뿐 아니라 정책 자체의 영구 강등 계약을 유지하며, 이후 시계가 다시 증가해도 이전 proof를 반환하지 않는다. 이미 호출자에게 전달되어 영속 commit된 원래 receipt를 소급해서 바꾸지는 않는다.

## 철회와 선형화

start/heartbeat/finish는 같은 AuthorityRegistry의 `with_authority` 안에서 짧은 순수 memory commit을 수행한다. 다른 registry/만료/교체/해제된 lease는 거절한다. entry가 lease를 보존하여 ticket TTL 동안 권위 수명을 유지한다. SessionInvalidator adapter는 **기존 authority를 먼저 철회하고** revoked ticket을 지운 뒤 그 실제 barrier를 반환한다. DB logout/삭제가 끝날 때까지 barrier를 유지하여 철회 전 generation의 late bind가 새 ticket을 만들지 못하게 한다. lock 순서는 authority→ticket이고 IO/async/blocking replay를 lock 안에서 실행하지 않는다. session hash를 ticket에 중복 복사하지 않는다.

후속 persistence에서는 sealed time의 account/attempt/metadata/digest를 실제 evidence에 다시 결합하고 기존PG account/session lock과 첫 완료 불변성을 유지한다. timing을 얻은 뒤 취소됐다고 이미 commit된 DB를 되돌렸다고 주장하지 않는다. 기존 completion을 나중에 time으로 승격하지 않고 같은 attempt/Practice는 원래 저장된 receipt를 반환한다.

리뷰의 실제 Red는 caller가 사라진 다른 계정의 만료 entry를 purge할 때 마지막 ConnectionAuthority Drop이 같은 AuthorityRegistry에 재진입하여 pure commit이 완료되지 않는 경로다. 제거·교체·clock 실패·철회 때 entry는 먼저 retired 목록으로 이동하고 **authority와 ticket lock 둘 다 해제한 뒤** drop한다. invalidator도 ticket lock 안에서 lease를 해제하지 않아 lock 역전을 방지한다. 단순히 retain/clear 안에서 drop하거나 잠금 시간을 늘려 해결하지 않는다.

## 검증

실제 Red→Green으로 시작 재시도/교체/capacity, exact countdown/45초/TTL, heartbeat duplicate/순번/영구강등, accepted rules/metadata, actual Native evidence와 server 최소 경과, finish 동시성/원래 digest, registry 재생성, clock 역행/overflow, account/hash 철회 barrier/late bind/다른 lease를 검사한다. 시간 정책과 adapter를 Daily95% 측정에 포함하고 기존 domain95/전체80 및 entry point 포함을 유지한다. 실제PG/HTTPS는 회귀로 실행하며 이 foundation의 공개 시간 기능 검증으로 계산하지 않는다. 통합 활성화·실계정42·물리25·하드웨어26·사람27·기존SW83과 상위19는 미완료다.
