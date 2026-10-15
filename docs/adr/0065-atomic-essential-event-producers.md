# ADR0065 · 결과와 필수 이벤트의 원자적 수집

- 상태: 채택 · 승인된 FR01/09/14/16 · 상위20/하위138
- 선행: #136 CLOSED·PR137/develop3a1e0be 병합 확인
- 시나리오: TS15/16/17/18/20/21/29/31/35/36 · NFR01/02/05/07/08

## 권위와 트랜잭션

온라인 direct repository와 단일 writer runtime, admission journal finish와 시작 시 unknown 복구, 데일리 첫 official 완료의 기존 source transaction 안에서 essential fact를 저장한다. 별도 pool acquire·중첩 transaction·공개 수집 endpoint를 만들지 않는다. 내부 helper는 같은 transaction의 원본 loader/receipt validator·ProductEvent·불변 insert/dedup을 재사용한다. source는 아직 외부에 보이지 않아도 같은 transaction에서 권위 있게 읽을 수 있다. 성공 ACK는 source와 해당 살아 있는 인간 참여자의 event가 함께 commit된 뒤에만 반환한다. 이벤트 INSERT 실패는 source INSERT·active 삭제까지 함께 rollback한다.

수집시각은 기존 서버 Clock의 transaction 시각이다. runtime은 명령을 받았을 때 저장한 recorded_at을 사용하며 DB 지연으로 게임 발생시각을 바꾸지 않는다. daily는 현재 계정 권한을 잠근 뒤의 Clock을 사용한다. client claim/seed/replay/PII를 event에 복제하지 않는다. optional analytics/advertising 쓰기는 계속 차단한다.

## 잠금·재시도·만료

온라인 writer는 account UUID 오름차순의 FOR UPDATE를 먼저 얻고 participant FK·source/event를 처리한다. direct는 source INSERT 전에, journal은 participant row 잠금 전에 얻는다. recovery batch는 최대64판의 계정을 전체 UUID순으로 먼저 잠가 판마다 반대 순서로 계정을 얻는 교착을 막는다. register도 같은 account 순서를 따른다. 기존 journal owner/2초 deadline/큐16/복구10000판 상한은 유지한다. daily는 기존 authorize_account_transaction의 account 잠금을 재사용한다.

삭제된 계정은 복원하지 않고 실제 남은 participant만 수집한다. bot에게 event를 만들지 않는다. 양쪽 인간은 각1건, 한 인간과 bot은 인간1건이다. 삭제 후 남은 인간의 상대 종류는 원본 loader가 결정한다. 이미 확정된 Duplicate·같은 daily attempt·후속 Practice는 collector를 호출하지 않아 최초 accepted ID/received_at을 유지한다. 원래30일이 지나 물리 삭제된 raw를 결과 재전송으로 다시 만들지 않는다. recovery의 원래 게임 보존90일이 끝났으면 오래된 journal을 닫되 expired event는 저장하지 않는다. 서버 장애 복구는 server_failure/abort의 미확정 사실이며 정상 완료/NSM으로 바꾸지 않는다.

## 관측과 남은 출구

실제 main 준비 실패를 구분하도록 admission의 board_take/prepare/register 실패는 stable component·stage·code만 내부 stderr에 기록한다. source ID·계정·정답·seed·입력·SQL 오류 원문을 포함하지 않는다. 실제 OS 시험은 서버 stderr를 격리 파일에 보존하고 준비 실패 시 해당 진단을 보여준다. 공개 error/timeout/재시도·게임 규칙을 바꾸지 않는다.

후속 CI의 강제 register 잠금 시험은 첫 보드 준비가 최대2초인 경로에서 바로1초 SQL 경계를 기다려 실패했다. 실제 BoardPool worker는 처음 인증된 보드를 cache에 넣은 뒤 stable ready 진단을1회만 남긴다. OS 시험의 기존8초 준비 상한 안에서 HTTP ready와 실제 cache ready를 함께 관측한 뒤 첫 요청을 시작한다. 실제 OsSeedSource/인증 생성·2초 admission/1초 SQL 경계·4초 pair 상한은 유지하며 cold 준비를 완료한 것으로 추정하지 않는다. 최초 public unavailable의 어느 단계인지는 추가 증거 없이 확정하지 않는다.

실제 PG로 자동1:1 저장·동시 반대좌석 account 순서·삭제 경합·source/event 공동 rollback·원본 dedup·정확30일/90일·daily first/Practice·유효 sealed time을 확인한다. 실제 main OS kill/restart의 unknown 저장에도 최소 event가 자동 생겨야 한다. HTTPS PC/mobile8언어 본인 다운로드는 test-only 수동 capture route 없이 자동 event를 관측한다. 기존 규칙·DTO·game/replay 보존·광고/언어 출구를 유지한다.

물리 bounded raw30일·anonymous calendar13개월 보존, authoritative first-normal/D7 완결성·운영 조회·상위20 통합 감사는 다음 필수 출구다. raw30일의 첫 관측을 lifetime first로 간주하지 않는다. 실제 CMP/실OAuth42·운영25/26·사람27·미확정83은 별도다.
