# ADR 0002: PostgreSQL

- 날짜: 2026-10-06
- 상태: 채택 — 사용자 구현 시작 지시, PR 병합 승인 별도

## 배경

홈서버의 결과·세션·순위에 transaction과 제약이 필요하다.

## 결정안

Docker 내부 PostgreSQL과 sqlx, 앱 DML 계정·migration 분리, parameter binding을 사용한다.

## 대안

SQLite는 고정 DB 조건과 다르고 hosted DB는 비용·범위 밖이다.

## 영향과 검증

백업·upgrade·disk 운영 필요. 동일인 첫 기록은 DB unique로 보장한다.

## 관련 설계

[상세 계약](../design/07-database.md)
