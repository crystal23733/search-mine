# 0039. 인증된 본인 완료 결과 조회

상태: 채택 · M3 #18/#82. 선행 #80/PR81, #85/PR86, #84/PR87 병합·종료 확인 뒤 설계부터 반영한다. FR14/16, NFR01/02/05/07 · TS15/16/20/21/31/35/36.

## 계약과 개인정보

`POST /api/v1/results`는 256byte 이하의 closed JSON `{v:1,match_id:canonical UUID}`만 받는다. query, 알 수 없는 필드, 중복 필드, nil/비정규 UUID와 지원하지 않는 버전을 거부한다. Rust 원천에서 생성한 TS 응답은 match UUID·저장된 rules hash·종료 elapsed·reason/outcome/completed·본인 opened_safe/mistakes/accusation_attempts/correct_accusations만 포함한다. 현재 rules snapshot을 과거 hash에 붙이지 않는다. seed·보드·상대 통계/식별자·provider·session은 없다. completed 판정은 기존 Rust 결과 투영과 공유한다.

## 읽기 경계와 보존

actor/registry와 독립된 `ResultReader` port를 둔다. PostgreSQL은 요청 match와 인증 계정의 참여 행을 함께 조건으로 조회한다. 공개 필드와 내부 본인 계정 검증값만 선택하며 seed를 읽지 않는다. 결과 기록 시각부터 90×86400초 미만만 조회하며 DB timezone/DST에 따라 조회 기간이 달라지지 않는다. 물리 정리 작업 #25의 완료를 의미하지 않는다. 없는 match·다른 계정·삭제된 참여 행·만료 결과는 같은 `not_found`다. 같은 계정의 중복 seat나 오염된 hash/elapsed/stats/reason/outcome은 `unavailable`로 닫는다. 저장 retry는 삭제한 참여 행을 복원하지 않는 기존 transaction을 유지한다.

## 인증과 자원 상한

Origin/CSRF/HttpOnly session·nickname·JSON Content-Type을 검증한다. 모든 응답은 no-store/no-referrer/nosniff다. `LIAR_RESULT_REQUESTS` 기본16(1..256), `LIAR_RESULT_AUTHORITIES` 기본64(1..20000), 전체 IO 단일2초 기한, session/account 각각 초당20회와 최대4096개/60초 이력을 적용한다. 장비 성능 실측값이 아니다.

조회 전용 AuthorityRegistry를 사용해 WS/로비 소유권과 분리한다. initial session read 전 generation을 캡처하고 bind_shared 후 DB 조회, session을 다시 읽어 동일 ID·account·created/expires/authenticated lifetime을 확인한다. 최종 최소 응답은 with_authority의 철회 lock 안에서 순수 연산으로 생성한다. IO는 lock 밖에서 수행한다. CombinedSessionInvalidator에 합성해 logout/삭제/세션 교체 중 오래된 응답을 차단한다. 취소는 permit/lease와 진행 중 future를 해제한다. 이미 전달한 정보의 철회를 약속하지 않는다.

## 한계와 검증

키 없는 서버는 unavailable이다. `not_found`를 pending/saved/forfeit/server_failure로 추정하지 않는다. 웹 소비·메모리 후보를 잃은 뒤의 match 발견·영속 active journal·재시작 abort는 후속 범위다. strict DTO/secret0, 전후 인증·철회·취소·상한·기한·rate·실제 PostgreSQL/HTTP·삭제와 exactly-once를 행동 중심 Red→Green으로 확인한다.
