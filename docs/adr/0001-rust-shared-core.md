# ADR 0001: Rust 공유 코어

- 날짜: 2026-10-06
- 상태: 채택 — 사용자 구현 시작 지시, PR 병합 승인 별도

## 배경

규칙 중복과 native/WASM 차이를 줄여야 한다.

## 결정안

Rust core crate를 native 서버와 wasm-bindgen 로컬 모드가 공유하고 golden fixture로 결정론을 검증한다.

## 대안

TS별도 구현은 중복, 서버 전용은 오프라인 요구를 충족하지 못한다.

## 영향과 검증

Rust/WASM toolchain 비용. 온라인 WASM에 secret seed를 전달하지 않는다.

## 관련 설계

[상세 계약](../design/03-domain-model.md)
