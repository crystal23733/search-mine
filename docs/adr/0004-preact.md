# ADR 0004: Preact UI

- 날짜: 2026-10-06
- 상태: 제안 — 사용자 설계 승인 대기

## 배경

PC/모바일 UI·작은 초기 번들·Atomic Design이 필요하다.

## 결정안

Preact+Vite의 서비스 interface/DI와 DOM 접근성 grid를 사용한다.

## 대안

React는 대안이지만 고정 권장 스택 변경에는 사용자 승인 필요; 순수 Canvas만은 접근성 부족.

## 영향과 검증

클라이언트 타입·state adapter와 code split 검증 필요.

## 관련 설계

[상세 계약](../design/08-client-architecture.md)
