# ADR 0005: PixiJS 게임 표시

- 날짜: 2026-10-06
- 상태: 채택 — 사용자 구현 시작 지시, PR 병합 승인 별도

## 배경

격자 렌더·이펙트를 재사용하면서 모바일 성능을 측정해야 한다.

## 결정안

PixiJS를 게임 진입 시 로드하고 dirty cell·공통 tokens·DOM 접근성 view를 사용한다.

## 대안

DOM-only는 대안, Phaser는 추가 엔진 기능이 필요하지 않은 현재 범위에서 보류한다.

## 영향과 검증

전체 게임 JS 초기200KiB 충족은 미확인. critical path/게임 청크 예산 분리 제안은 승인 필요.

## 관련 설계

[상세 계약](../design/13-performance.md)
