# ADR 0008: develop과 이슈별 검토 PR

- 날짜: 2026-10-06
- 상태: 제안 — 사용자 설계 승인 대기

## 배경

사용자가 기본 develop·마일스톤/이슈·완료 PR·리뷰 후 병합을 지정했다.

## 결정안

develop에서 type/issue-slug 분기, Conventional Commits, develop 대상 PR, squash merge 후 이슈 종료, 다음 작업 진행.

## 대안

기존 Test|/Chore|와 feature/front-atoms/develop 중첩 관례는 최신 요청에 맞춰 교체한다.

## 영향과 검증

1인 소유자는 자기 PR 공식 approval 불가. review count0+checks+사용자 수동 검토/명시 승인으로 운영하며 자동 병합하지 않는다.

## 관련 설계

[상세 계약](../workflow/git-workflow.md)
