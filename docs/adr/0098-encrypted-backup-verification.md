# ADR0098: 암호화 백업과 격리 복원 검증

- 상태: 승인된 FR16 운영 설계의 구현 결정
- 이슈: [#213](https://github.com/crystal23733/search-mine/issues/213), 상위 [#25](https://github.com/crystal23733/search-mine/issues/25)
- 선행: #211/PR212 병합·종료. 외부 호스트 출구 #24와 분리한 개발 범위다.
- 요구: FR16, NFR02/05/07/08 · TS28/29/31/35

## 결정

PostgreSQL18 클라이언트와 age 공식 최신 안정 소스로 만든 별도 rootless 도구 이미지를 사용한다. backup profile은 DB 내부망만 연결하고 owner 비밀번호 파일과 age 공개 recipient 파일만 읽는다. 개인키·Apple vault key·CSRF key는 백업 이미지/볼륨에 넣지 않는다. pg_dump custom 출력을 age stdin으로 바로 연결하고 두 프로세스 성공을 모두 확인한다. 평문 dump 파일은 만들지 않는다. stderr는 비밀을 포함할 수 있으므로 고정 오류만 출력한다.

동일 저장소 작업은 flock으로 직렬화한다. 시작 UTC epoch와 UUID로 관리 디렉터리를 식별한다. private pending 디렉터리에 암호문·manifest를 작성한 뒤 atomic rename으로 게시한다. manifest는 형식 버전·원래 시작시각·암호문 크기·SHA256만 포함한다. checksum은 매체 손상 검사이고 발신자 인증은 아니다. 신뢰하는 자체 저장소/이미지/키만 사용한다. 공개키 암호화만으로 백업 출처를 인증한다고 주장하지 않는다.

보존 상한은 원래 시작시각에서 정확28일이며 미래 시각·변조된 이름/manifest·링크는 거절한다. prune은 도구가 생성한 이름·고정 파일 집합만 삭제하고 알 수 없는 파일을 재귀 삭제하지 않는다. 실패/중단 pending도 원래 시작시각 상한을 따른다. 자동 backup 앞에 prune을 실행하며 매일 실행 성공과 별도 매체 복사를 운영자가 감시한다. 일7/주4는 선택적 보존 빈도이며28일 상한보다 우선하지 않는다.

verify는 네트워크 없이 checksum/크기/보존을 확인하고 전체 age 인증을 끝낸 뒤 pg_restore 목록을 확인한다. 평문은 RO 이미지의 용량 제한 tmpfs에서만 임시로 보관하고 종료 시 제거한다. tmpfs보다 큰 dump는 실패로 보고하며 용량 산정은 #26/운영 검증 대상이다. verify는 DB에 접속하거나 서비스를 재개하지 않는다.

통합 검증만 고유 Compose의 빈 DB에 현재 전체 migrations와 합성 데이터를 복원한다. pg_restore는 --single-transaction/--exit-on-error/--no-owner/--no-privileges를 사용한다. 원래 보존 시각·FK·대표 데이터와 실패 시 빈 대상 유지를 확인한다. 테스트 외 복구 쓰기 명령을 제공하지 않는다.

## 서비스 복구의 남은 차단 조건

과거 dump의 tombstone만으로 최신 삭제를 알 수 없다. 최신 삭제 기록의 별도 내구 저장·재적용, 세션/거래/동의 무효화, 만료 원본/집계 정리와 metric_collection.coverage_since 보수적 전진, 단일 owner 시작/ready 차단, 이전 실제 release image 호환 검증이 충족되기 전 서비스 접근을 열지 않는다. #25에 남긴다. 실제 별도 매체·호스트 RPO≤24h/RTO≤2h도 본 자동 테스트 결과로 완료 처리하지 않는다. 개인 데이터 파일 다운로드/API는 만들지 않는다.

## 근거

- [PostgreSQL pg_dump](https://www.postgresql.org/docs/current/app-pgdump.html)
- [PostgreSQL pg_restore](https://www.postgresql.org/docs/current/app-pgrestore.html)
- [age 공식 설치와 사용](https://github.com/FiloSottile/age)
