# 복구 대상 격리와 권리 원장 재적용

[#217](https://github.com/crystal23733/search-mine/issues/217) · [ADR0100](../adr/0100-recovery-quarantine.md) · [검증](../verification/217-recovery-quarantine.md).

## 현재 제공 범위

이 도구는 별도 복구 대상의 quarantine과 삭제/연결 해제 원장 적용만 제공한다. **서비스 활성화 명령은 없다.** 최신 원장의 독립 내구 저장·완전한 cutover·이전 release 호환·원래 보존 정리·재개 검증이 완료되기 전 서비스 접근을 열지 않는다. 원장 입력이 최근5분 안에 생성됐다는 검사만으로 완전한 최신 원장임을 증명할 수 없다. 실제 재개/RPO/RTO 출구는 #25에 남는다.

관리 백업은 gate DATA를 복사하지 않는다. 과거 gate 없는 백업도 migration만으로 활성화되지 않는다. 실제 빈 schema에서 시작해 제품 테이블도 빈 최초 설치만 migrate가 active를 초기화한다. 기존 실제 DB의 최초 도입에는 별도 검증이 필요하며 gate를 직접 SQL로 active로 바꾸는 우회 절차를 사용하지 않는다. 빈 설치 migration이 중단돼 초기화 증거를 잃은 경우에도 자동 활성화하지 않는다. 기존 데이터를 담은 볼륨을 삭제하거나 빈 설치로 가장하지 않는다.

## 별도 대상 준비

[암호화 백업](encrypted-backups.md)의 검증을 통과한 신뢰하는 자체 archive와, 원래 DB/서비스와 구분한 빈 복구 대상을 사용한다. 실제 운영 DB를 대상으로 도구를 시험하지 않는다. `/restricted/restore-target.env`는 별도 복구 프로젝트/비밀 경로와 그 대상에만 연결되는 migration owner URL 파일을 지정한다. 같은 이름의 production env나 volume을 재사용하지 않는다.

이 문서는 pg_restore 실행이나 서비스 release를 자동화하지 않는다. gate를 검사하지 않는 과거 server 이미지는 복구 대상에 기동하지 않는다. 그러한 이미지의 안전한 rollback 호환성을 이번 검사로 인정하지 않는다. 현재 관리 도구로 생성한 backup의 복원·migration 후에는 gate가 없거나 격리 상태이므로 main이 HTTP를 열지 않는 것이 정상이다. 복구 대상에 server/web/Tunnel을 시작하지 않고, 아래 owner job만 DB 내부망에 연결한다. 살아 있는 journal owner가 있으면 도구가 실패하며 그 owner를 강제 종료하지 않는다.

```sh
docker compose -p liar-recovery --env-file /restricted/restore-target.env -f deploy/compose.yml --profile recovery run --rm --no-deps -T recovery
```

기본 명령은 quarantine이다. 고정 state와 임의 recovery_id를 반환한다. 이 UUID는 사용자 계정 ID가 아니라 해당 복구 건의 식별자다. 같은 대상의 이미 격리된 건은 같은 UUID를 돌려준다. DB/schema가 다르거나 원장이 다른 건을 가리키면 apply를 거절한다. owner 비밀번호/URL은 파일→자식 환경으로만 전달되고 argv/로그에 출력하지 않는다.

## 검증한 원장 적용

원장은 현재의 권위 있는 account/identity 삭제 기록으로 준비해야 한다. 현재 source는 account 삭제 tombstone과 identity 연결 해제/Apple 철회의 최소 UUID·원래 시각·28일 expiry를 DB에 보관한다. 과거 dump의 이 표를 최신 원장으로 간주하지 않는다. 별도 매체의 완전한 캡처/암호화 전달 도구와 재개 증거는 #25 후속이다.

입력 계약은 version=1, 해당 recovery_id, 실제 captured_at(epoch초), entries다. 각 entry는 kind(account/identity), id(UUID), deleted_at, expires_at만 가진다. 최대500개·128KiB, capture는 현재보다 미래가 아니며5분 이내, expiry는 deleted_at+2419200초이며 현재 아직 만료되지 않아야 한다. 중복/nil/알 수 없는 필드·미래 삭제는 전체 입력을 거절한다. 시각을 임의로 바꿔 오래된 자료를 새 자료로 가장하지 않는다.

암호화 원장은 age 전체 인증이 성공한 뒤에만 별도 제한 tmpfs의 검증된 JSON으로 전달한다. `age ... | recovery apply`처럼 인증 완료 전에 SQL 도구로 바로 흘려보내지 않는다. 아래 `/run/recovery-private/verified-ledger.json`은 이미 신뢰성과 인증·현재 원본과의 관계를 확인한 파일을 뜻한다. 개인 데이터 다운로드 화면/API를 제공하는 흐름이 아니다.

```sh
docker compose -p liar-recovery --env-file /restricted/restore-target.env -f deploy/compose.yml --profile recovery run --rm --no-deps -T recovery recovery apply < /run/recovery-private/verified-ledger.json
```

적용은 한 transaction에서 tombstone/계정·연결 삭제와 세션·거래·동의/선택 이벤트·오래된 Apple credential/철회 queue/receipt 제거를 수행한다. 제공자 API는 호출하지 않는다. collection 시작/익명 expiry는 유지하고 coverage를 검증 시각으로 전진시킨다. 실패하면 전체 rollback한다. 성공은 적용한 원장 항목 수와 quarantined 상태만 출력하며 **서비스 재개 승인이 아니다**. 최신 연결 추가/계정 상태·키 연속성·원래 보존 정리/owner/호환·완전한 cutover 검증이 남아 있다. 검증 tmpfs 파일은 작업 후 해당 파일만 정리하고 원장 사본에도 원래28일 상한을 적용한다.

## 내부 검사

main은 기동 전과 owner 확보 후 gate를 확인한다. 준비 상태도 gate가 active·같은 DB/schema인지 검사한다. serving 역할은 gate SELECT만 가지며 owner CLI만 수정할 수 있다. 테스트는 고유 합성 PG와 실제 main/CLI·관리 dump/빈 DB 복원으로 수행한다. 실제 운영자 키·매체·호스트/RPO/RTO를 검사 성공으로 보고하지 않는다.
