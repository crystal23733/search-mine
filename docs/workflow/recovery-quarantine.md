# 복구 대상 격리와 권리 원장 재적용

[#217](https://github.com/crystal23733/search-mine/issues/217) · [ADR0100](../adr/0100-recovery-quarantine.md) · [검증](../verification/217-recovery-quarantine.md).

## 현재 제공 범위

이 도구는 원본 권리 원장의 일관된 캡처와 별도 복구 대상의 quarantine·삭제/연결 해제 원장 적용을 제공한다. **서비스 활성화 명령은 없다.** 최신 원장의 독립 내구 저장·완전한 cutover·이전 release 호환·원래 보존 정리·재개 검증이 완료되기 전 서비스 접근을 열지 않는다. 원장 입력이 최근5분 안에 생성됐다는 검사만으로 완전한 최신 원장임을 증명할 수 없다. 실제 재개/RPO/RTO 출구는 #25에 남는다.

관리 백업은 gate DATA를 복사하지 않는다. 과거 gate 없는 백업도 migration만으로 활성화되지 않는다. 실제 빈 schema에서 시작해 제품 테이블도 빈 최초 설치만 migrate가 active를 초기화한다. 기존 실제 DB의 최초 도입에는 별도 검증이 필요하며 gate를 직접 SQL로 active로 바꾸는 우회 절차를 사용하지 않는다. 빈 설치 migration이 중단돼 초기화 증거를 잃은 경우에도 자동 활성화하지 않는다. 기존 데이터를 담은 볼륨을 삭제하거나 빈 설치로 가장하지 않는다.

## 별도 대상 준비

[암호화 백업](encrypted-backups.md)의 검증을 통과한 신뢰하는 자체 archive와, 원래 DB/서비스와 구분한 빈 복구 대상을 사용한다. 실제 운영 DB를 대상으로 도구를 시험하지 않는다. `/restricted/restore-target.env`는 별도 복구 프로젝트/비밀 경로와 그 대상에만 연결되는 migration owner URL 파일을 지정한다. 같은 이름의 production env나 volume을 재사용하지 않는다.

이 문서는 pg_restore 실행이나 서비스 release를 자동화하지 않는다. gate를 검사하지 않는 과거 server 이미지는 복구 대상에 기동하지 않는다. 그러한 이미지의 안전한 rollback 호환성을 이번 검사로 인정하지 않는다. 현재 관리 도구로 생성한 backup의 복원·migration 후에는 gate가 없거나 격리 상태이므로 main이 HTTP를 열지 않는 것이 정상이다. 복구 대상에 server/web/Tunnel을 시작하지 않고, 아래 owner job만 DB 내부망에 연결한다. 살아 있는 journal owner가 있으면 도구가 실패하며 그 owner를 강제 종료하지 않는다.

```sh
docker compose -p liar-recovery --env-file /restricted/restore-target.env -f deploy/compose.yml --profile recovery run --rm --no-deps -T recovery
```

기본 명령은 quarantine이다. 고정 state와 임의 recovery_id를 반환한다. 이 UUID는 사용자 계정 ID가 아니라 해당 복구 건의 식별자다. 같은 대상의 이미 격리된 건은 같은 UUID를 돌려준다. DB/schema가 다르거나 원장이 다른 건을 가리키면 apply를 거절한다. owner 비밀번호/URL은 파일→자식 환경으로만 전달되고 argv/로그에 출력하지 않는다.

## 검증한 원장 적용

원장은 현재의 권위 있는 account/identity 삭제 기록으로 준비해야 한다. 현재 source는 account 삭제 tombstone과 identity 연결 해제/Apple 철회의 최소 UUID·원래 시각·28일 expiry를 DB에 보관한다. 과거 dump의 이 표를 최신 원장으로 간주하지 않는다. 암호화 checkpoint와 전체 인증 후 전달 도구는 [#227 절차](encrypted-rights-ledger.md)를 따른다. 실제 별도 매체·최신 완전성/재개 증거는 #25 후속이다.

기존 수동 apply의 Ledger 입력 계약은 version=1, 해당 recovery_id, 실제 captured_at(epoch초), entries다. 각 entry는 kind(account/identity), id(UUID), deleted_at, expires_at만 가진다. 최대500개·128KiB, capture는 현재보다 미래가 아니며5분 이내, expiry는 deleted_at+2419200초이며 현재 아직 만료되지 않아야 한다. 중복/nil/알 수 없는 필드·미래 삭제는 전체 입력을 거절한다. 시각을 임의로 바꿔 오래된 자료를 새 자료로 가장하지 않는다.

암호화 원장은 age 전체 인증이 성공한 뒤에만 별도 제한 tmpfs의 검증된 JSON으로 전달한다. `age ... | recovery apply`처럼 인증 완료 전에 SQL 도구로 바로 흘려보내지 않는다. 아래 `/run/recovery-private/verified-ledger.json`은 이미 신뢰성과 인증·현재 원본과의 관계를 확인한 파일을 뜻한다. 개인 데이터 다운로드 화면/API를 제공하는 흐름이 아니다.

```sh
docker compose -p liar-recovery --env-file /restricted/restore-target.env -f deploy/compose.yml --profile recovery run --rm --no-deps -T recovery recovery apply < /run/recovery-private/verified-ledger.json
```

적용은 한 transaction에서 tombstone/계정·연결 삭제와 세션·거래·동의/선택 이벤트·오래된 Apple credential/철회 queue/receipt 제거를 수행한다. 제공자 API는 호출하지 않는다. collection 시작/익명 expiry는 유지하고 coverage를 검증 시각으로 전진시킨다. 실패하면 전체 rollback한다. 성공은 적용한 원장 항목 수와 quarantined 상태만 출력하며 **서비스 재개 승인이 아니다**. 최신 연결 추가/계정 상태·키 연속성·원래 보존 정리/owner/호환·완전한 cutover 검증이 남아 있다. 검증 tmpfs 파일은 작업 후 해당 파일만 정리하고 원장 사본에도 원래28일 상한을 적용한다.

## 내부 검사

main은 기동 전과 owner 확보 후 gate를 확인한다. 준비 상태도 gate가 active·같은 DB/schema인지 검사한다. serving 역할은 gate SELECT만 가지며 owner CLI만 수정할 수 있다. 테스트는 고유 합성 PG와 실제 main/CLI·관리 dump/빈 DB 복원으로 수행한다. 실제 운영자 키·매체·호스트/RPO/RTO를 검사 성공으로 보고하지 않는다.

## 일관된 원본 캡처와 출처 검증 (#225)

[ADR0103](../adr/0103-recovery-source-snapshot.md) / [검증225](../verification/225-recovery-source-snapshot.md)의 native owner 경로는 `recovery capture`와 `recovery apply-snapshot <incident UUID>`다. source UUID는 gate/복구 incident와 구분하며 관리 dump에 포함된다. 정상 active DB 업그레이드 또는 최초 빈 설치에서만 초기화한다. 과거 출처 없는 백업을 다른 출처로 가장하거나 격리 대상을 자동 초기화하지 않는다.

capture는 live main과 병행 가능한 read-only/repeatable-read transaction이다. 원래 삭제·연결 해제 UUID와 시각만 읽고, 미확정 변경은 포함하지 않는다. 두 표 합계500개/128KiB와2초 한도를 초과하거나 gate·시각이 잘못되면 부분 결과 대신 실패한다. 전체 current capture의 일관성은 이후 권리 변경의 영구 보관/최신 완전성을 뜻하지 않는다.

capture stdout에는 개인 UUID가 포함된다. 터미널/로그/평문 archive로 남기거나 사용자 화면/API에 연결하지 않는다. **독립 암호화 checkpoint와 전체 인증 후 SQL 전달 도구는 [#227 절차](encrypted-rights-ledger.md)를 따른다. 실제 매체/주기·모든 권리 변경의 최신 완전성은 #25 후속이다.** 현재 native 경로를 운영용 자동 보관·복구 절차가 완성됐다고 해석하지 않는다. `age --decrypt | recovery apply-snapshot ...`처럼 인증 전에 DB를 바꾸는 직접 pipe도 사용하지 않는다.

snapshot은 version/source_id/captured_at/entries를 보존한다. 실제 capture 시각을5분 뒤에 현재로 덮어쓰지 않으며 최대28일까지만 검증한다. apply-snapshot은 인증된 입력을 메모리/stdin으로 받아 현재 quarantine incident·DB/schema와 보존된 source UUID가 모두 맞는지 확인한다. capture 때 유효했던 항목 중 현재 만료된 것은 재적용하지 않으며, 아직 유효한 항목의 원래 expiry는 연장하지 않는다. 삭제/세션 무효화 절차는 기존 apply와 공유하고 성공도 quarantine 상태다. source UUID가 같다는 사실만으로 최신 데이터 완전성이나 서비스 재개를 허가하지 않는다.

#231의 통제된 복구 재개는 [최신 원본 차단 절차](encrypted-rights-ledger.md)를 따른다. source `sealed`는 일반 quarantine/SQL 상태 변경으로 다시 열지 않으며, 최신 자료를 확인할 수 없는 장애에서 자동 재개하지 않는다.
