# ADR 0009: OAuth 전용 인증과 최소 계정 정보

- 날짜: 2026-10-06
- 상태: 채택 — 사용자 변경·개발 지시, [근거](../workflow/approval-record.md). PR 병합 승인은 별도.

## 배경

사용자가 기존 익명 게스트 설계를 바꾸고 비밀번호를 보관하지 않는 OAuth와 필요한 최소 정보를 요구했다. Google·Apple·카카오·네이버는 첫 공개 버전의 필수 제공자이며 이후 추가도 가능해야 한다.

## 결정

온라인 계정은 OAuth 전용, 로컬 봇/튜토리얼/데일리 연습은 계정 없이 제공한다. Authorization Code 서버 교환, provider port/registry와 개별 adapter로 검증 계약을 분리한다. Google/Kakao openid만, Apple name/email scope 없음, Naver 기본 id만. 내부 UUID·게임용 nickname·issuer와 subject digest로 계정을 식별하고 불필요한 개인 정보는 요청/저장하지 않는다.

비밀번호·password hash·이메일 회원가입/복구를 만들지 않는다. 제공자 token은 브라우저에 주지 않고 자체 opaque HttpOnly 세션을 쓴다. Apple 철회용 refresh token만 서버에 암호화 보관하며 비밀번호 미보관과 구분한다. 이메일/닉네임 자동 병합 없이 명시적 재인증 연결을 사용한다.

## 대안과 영향

기존 서버 게스트 세션은 채택하지 않는다. 로컬 무계정 연습은 유지한다. 모든 제공자에게 같은 scope/PKCE를 강제하는 방법은 서로 다른 계약 때문에 사용하지 않는다. 가입 마찰·공급자 장애·등록/검수 부담이 생기며 E1/E2 관찰과 로컬 대체 경로로 평가한다.

## 검증

[17 인증/개인정보](../design/17-auth-privacy.md), [07 DB](../design/07-database.md), [06 API](../design/06-protocol.md), TS01/20/29/31~36. #15는 4개 실제 어댑터·실DB·계정 lifecycle을 완료해야 닫는다. 운영 법적 문구와 OAuth 등록/키는 공개 전 #21 및 사용자 설정 절차로 확인한다.
