# #213 암호화 백업 검증

FR16 · NFR02/05/07/08 · TS28/29/31/35 → [ADR0098](../adr/0098-encrypted-backup-verification.md).

## 인수 시나리오

1. 현재 전체 migration과 합성 계정/삭제 tombstone/보존 시각을 가진 격리 PG18에서 backup한다. 암호문·manifest만 게시되고 host 평문/key/로그 노출 없이 verify와 별도 빈 DB 복원에 성공한다. row/FK/원래 시각을 비교한다.
2. DB 접근 실패·잘못된 recipient·암호화 소비자 실패는 완료 백업을 남기지 않는다. 프로세스 양쪽 상태와 pending 정리를 확인한다.
3. 암호문 변조·잘린 파일·manifest 크기/checksum 변조·다른 개인키·만료·미래 시각·심볼릭 링크는 verify 실패다. age 전체 인증 실패 시 pg_restore를 실행하지 않는다.
4. 정확28일 경계에서 정상/pending 관리 자료만 정리한다. 임의 파일/링크/시각 불일치가 있으면 보존하고 실패한다. 원래 시각을 파일 mtime으로 연장하지 않는다. 동시 작업은 flock으로 거절한다.
5. 실제 도구는 rootless/RO/cap-drop·backup DB망/verify network-none·공개 포트0이다. 합성 개인키는 fixture에서만 만들고 종료 시 고유 container/network/volume와 함께 제거한다. CI artifact는 비밀 없는 summary/build log만 올린다.

## 실행 기록

시나리오 선행 후 `python3 -m unittest discover -s deploy/tests -p test_backup.py` Red를 실제 실행했다. 구현 파일 미존재 실패 원문은 로컬 `.tmp/backup213-red.log`에 보존했다. Green은16개 계약 테스트 통과다. 실제 PG18.6/age1.3.2 이미지로 암호화 생성→network-none verify→별도 빈 DB 단일 transaction 복원에 성공했다. 전체 migration checksum·계정/identity/tombstone·metric collection/익명 집계 원래 만료시각·FK를 비교하고 복원 후 expiry 불변/coverage 역행 거절을 확인했다.

잘린/변조 암호문의 외부 checksum을 다시 맞춘 경우에도 age 인증이 실패했고, 실패 대상 DB 테이블0을 확인했다. 다른 개인키·실제 DB 비밀번호 오류·invalid recipient를 거절했고 실패한 backup의 pending/완료 자료 증가0을 확인했다. 고유 fixture container/network/volume/합성 secret와 암호문을 정리하고 summary의 cleanup_verified를 확인했다. 코드 리뷰에서 rootless `/app` traverse 권한, Linux secret 파일별 mount, FIFO/nonblocking lock, 완료 manifest 직후 중단 pending, 일회성 container 소유 라벨/anonymous PG volume 방지를 보강했다. 기존 제품 timeout/retry/flags 변경은 없다.

문서260/Mermaid34/diff 검사 통과. Rust/TS 제품 코드는 변경하지 않았으며 전체 제품 검사와 신규 backup CI의 최종 head/실제 artifact 결과는 PR에서 별도 확인한다. 실서비스 복구/이전 release image/별도 매체/실호스트 RPO/RTO는 미실행이며 상위 #25를 유지한다.
