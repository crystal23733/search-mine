# 0045. 실제 결과 저장 시각과 보존 기준 분리

상태: 채택 · M3 #18/#99 · FR14/16, NFR01/02/05/07/08 · TS15/16/20/21/31/35/36. 선행97/PR98 developecb6f21 병합·종료 확인.

## 의무

현재 결과의 recorded_at은 실제 저장 시각이며 reader는 이 시각으로90일 미만 조회를 제한한다. 후속 journal의 늦은 startup abort가 저장 시각을 새로 찍으면 과거 참여 개인정보의 보존 기간을 다시 시작하게 된다. 저장 시각을 입장 시각으로 꾸며내거나 미확인 종료 시각을 추정하지 않고 별도 보존 기준을 둔다.90일은 기존 프로젝트 정책이며 법정 의무 기간이라고 주장하지 않는다. 물리 정리/백업 삭제는25의 별도 출구다.

## 결정

forward migration으로 online_match_results.retention_started_at TIMESTAMPTZ NULL을 추가하고 기존 행은 recorded_at으로 backfill한다. NULL 또는 epoch0~9999년 범위/recorded_at 이하의 유한 시각만 CHECK로 허용한다. DEFAULT NULL로 기존 INSERT와 호환하며 이전 migration checksum·기존 index는 유지한다. 미래 cleanup을 위해 COALESCE(retention_started_at,recorded_at) 표현식 index를 추가한다.

정상 writer는 같은 AuthClock 시각을 recorded_at과 retention_started_at에 명시 저장한다. 기존 UUID duplicate는 parent/participant/시각을 다시 쓰지 않고 삭제 행도 복원하지 않는다. reader는 COALESCE(anchor,recorded_at)으로 정확7776000초 미만을 판정하고 실제 recorded_at의 미래 값과 지원 범위 밖 anchor는 조회하지 않는다. NULL fallback은 구버전 INSERT의 기존 recorded_at 기준을 유지한다. DST/calendar90days로 정책을 바꾸지 않는다. 공개 DTO/known JSON/8언어 화면에는 새 시각/개인정보를 노출하지 않는다. secret seed/seed_expires_at 계산·보존 정책은 이번에 바꾸지 않는다.

후속 journalled final/abort는 알려진 admitted_at을 anchor로 쓰고 recorded_at은 실제 저장 시각으로 유지한다. 이번 작업에는 journal/admission/startup abort·OS kill/restart 복구·새로고침 발견이 없으며 상위18을 닫지 않는다. 삭제/백업25는 같은 anchor를 사용해야 하고90일을 넘긴 사용자 연계 정보가 물리 삭제됐다고 주장하지 않는다.

## 검증

schema 미지원 Red→migration Green 뒤 기존 reader가 최근 저장/오래된 anchor를 노출하는 행동 Red→reader/writer Green을 구분한다. 기존 실제 known JSON을 capture한 historical fixture로 구 스키마 INSERT/forward upgrade의 공개 계약을 확인한다. 현재 writer/reader가 오래된 schema와도 작동한다고 가정하지 않는다. 실제 PG의 backfill/NULL fallback·exact90일/DST·최근 recorded_at/오래된 anchor·미래/음수/무한/역행 CHECK·duplicate/삭제·기존 seed 시각 유지·권한 HTTP를 검증하고 domain95/전체80과 관련 검사를 기록한다. 코드 리뷰/최신 CI/head 후 위임 병합한다.
