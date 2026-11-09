# 운영자 암호화 백업

[#213](https://github.com/crystal23733/search-mine/issues/213) · [ADR0098](../adr/0098-encrypted-backup-verification.md) · [검증](../verification/213-encrypted-backup-verification.md). 개인 파일 다운로드 화면과 무관하다. 이 도구는 백업 생성/검사/보존 정리만 제공하고 서비스 복구 명령은 제공하지 않는다.

## 준비

[컨테이너 배포](container-deployment.md)의 PostgreSQL migration 완료 후 사용한다. `deploy/.env.example`의 공개 경로 설정을 운영 env 파일에 복사한다. `LIAR_BACKUP_DIR`는 별도 매체에 실제 마운트한 디렉터리여야 하며 UID10001만 쓰도록 권한을 맞춘다. 같은 디스크의 다른 폴더는 장애 복구 매체로 인정하지 않는다. 마운트가 해제된 상태에서 기본 디스크 폴더에 쓰지 않게 호스트 스케줄러에서 `mountpoint`/파일시스템 ID를 확인한다. 확인 실패 시 실행하지 않는다.

백업 전용 age identity는 신뢰하는 오프라인 환경에서 생성하고 복구 가능한 별도 보관소에 저장한다. `age-keygen -o /offline/restricted/backup_identity`로 파일을 만들고 `age-keygen -y /offline/restricted/backup_identity`의 **공개 recipient**만 운영 `LIAR_SECRETS_DIR/backup_recipient`로 전달한다. 개인키가 유실되면 백업은 복호화할 수 없다. 복호화 identity는 상시 백업 서버에 두지 않는다. Apple vault/digest/CSRF/OAuth 키와 DB 비밀번호도 dump에 포함되지 않으므로 별도 비밀 복구 절차를 유지한다.

secret 파일은 실제 컨테이너 UID10001에 읽기 권한을 부여한다. 배포 문서의0400/0440와 UID 매핑 검사를 따른다. 암호화 파일은0600, 관리 디렉터리는0700이다. 저장소 자체도 접근을 제한한다. 사용자 identity/계정/키로 자동 테스트하지 않는다.

## 생성과 정리

아래 명령에서 `/restricted/production.env`를 실제 공개 메타데이터/경로 env 파일로 바꾼다. Compose 프로젝트명과 배포 파일은 기존 운영 배포와 동일하게 유지해야 DB 내부망을 공유한다. 별도 자동 배포나 외부 계정 설정은 하지 않는다.

```sh
docker compose --env-file /restricted/production.env -f deploy/compose.yml -f deploy/backup.compose.yml --profile backup build backup
docker compose --env-file /restricted/production.env -f deploy/compose.yml -f deploy/backup.compose.yml --profile backup run --rm --no-deps backup
```

첫 명령으로 검토한 이미지 ID/digest와 `age --version`/`pg_dump --version`을 배포 기록에 고정한다. 운영에서는 그 immutable 이미지 참조를 `LIAR_BACKUP_IMAGE`에 설정하고 임의로 재빌드하지 않는다. 성공 출력은 고정 status와 백업 이름뿐이다. 비밀번호는 owner 파일에서 자식 환경으로 전달하고 명령 인수나 manifest/로그에 넣지 않는다. 백업은 DB 내부망만 사용하며 공개 포트를 만들지 않는다.

호스트 스케줄러에서 매일 위 생성 명령을 실행하고 종료코드0/최근 완료시각/별도 매체 마운트를 감시한다. backup은 먼저 prune한다. 생성이 중단돼도 정리 시각이 밀리지 않도록 다음 독립 정리도 매일 실행한다. flock 경합은 실패하므로 중복 스케줄을 만들지 않는다.

```sh
docker compose --env-file /restricted/production.env -f deploy/compose.yml -f deploy/backup.compose.yml --profile backup-prune run --rm --no-deps backup-prune
```

도구는 실행 시 원래 시작시각에서 정확28일 이상 지난 관리 백업/pending을 삭제하고 verify도 거절한다. 스케줄러 중단 중의 물리 삭제는 보장할 수 없으므로 운영에서는 여유를 둔 정리 주기·실패 경보와 복제본 삭제를 함께 검수한다. 실제 매체의28일 상한 검증은 #25에 남는다. 파일 mtime을 바꿔 연장하지 않는다. 일7개/주4개는 그 안의 선택적 빈도이며 최대28일 상한을 넘길 수 없다. 다른 매체/복제본에도 같은 상한을 적용해야 한다. malformed manifest·미래 시각·링크·알 수 없는 파일이 있으면 정리를 중단한다. 실패 경보를 확인한 운영자가 해당 관리 디렉터리와 생성시각을 대조해 처리하며 도구가 임의 사용자 파일을 재귀 삭제하지 않는다. SIGKILL/정전의 손상 pending도 이 확인 대상이다.

## 네트워크 없는 검사

복사한 신뢰하는 저장소를 별도 검사 환경에 마운트하고, 그 환경의 restricted secrets에 `backup_identity`만 제공한다. 동일 도구는 verify에 DB 비밀번호를 마운트하지 않는다. 개인정보를 포함한 평문은 제한된 `/tmp` tmpfs에만 복호화한다. 기본1GiB보다 큰 archive는 실패하므로 실제 DB 크기에 맞춰 검사 환경의 tmpfs/메모리 한도를 먼저 산정한다. 호스트 swap 정책/검사 장비 접근 통제도 운영 검수 사항이다.

```sh
docker compose --env-file /restricted/production.env -f deploy/compose.yml -f deploy/backup.compose.yml --profile backup-verify run --rm --no-deps backup-verify verify --name backup-START-UUID
```

`backup-START-UUID`는 실제 생성 결과의 이름으로 바꾼다. 크기/SHA256 검사를 통과해도 age 전체 인증이 끝나기 전 pg_restore를 실행하지 않는다. 인증 후 archive 목록까지 읽어야 verified를 출력한다. 검사값은 손상 탐지이고 출처 서명은 아니므로 신뢰하는 자체 백업만 검사한다. 이미지가 제공하는 복구 SQL 실행 명령은 없다.

## 복구 출구

자동 검증은 합성 데이터를 담은 고유 PG18의 빈 DB에 단일 transaction으로 복원하고 migrations/FK/원래 시각을 비교한다. 이는 실제 운영 RPO/RTO나 이전 release 호환성 측정이 아니다. 실제 서비스 복구 전에 최신 삭제 tombstone의 독립 내구 기록 재적용, 세션/거래/동의 무효화, 원래 만료시각의 정리와 metric coverage 보정, owner/ready 차단, 이전 실제 이미지·대표 API/순위 검증이 필요하다. 최신 삭제 기록을 확보할 수 없으면 접근을 열지 않는다. 이 출구와 별도 매체·실제 호스트 훈련은 상위 #25에 남는다.

관리 dump는 service_recovery_control DATA를 제외한다. 복구 대상의 차단/원장 적용은 [복구 격리 절차](recovery-quarantine.md)를 따르며 apply 성공도 서비스 활성화를 뜻하지 않는다. 기존 gate 없는 백업의 migration도 자동 활성화하지 않는다.
