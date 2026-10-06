# ADR 0003: 홈서버와 Cloudflare Tunnel

- 날짜: 2026-10-06
- 상태: 제안 — 사용자 설계 승인 대기

## 배경

전기/도메인 수준 현금비, 집IP·포트 미공개가 고정 조건이다.

## 결정안

한 미니 PC Docker Compose, outbound Tunnel, nginx/server/DB host port 미공개.

## 대안

유료 클라우드와 직접 포트포워딩은 제외 조건에 충돌한다.

## 영향과 검증

단일 장애점·글로벌 RTT·회선 약관·실측 용량. 오프라인 캐시와 복구 훈련을 마련한다.

## 관련 설계

[상세 계약](../design/02-deployment-network.md)
