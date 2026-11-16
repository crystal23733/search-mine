# 14. 제품 이름

> 적용: pm-marketing-growth:product-name · 입력: [GTM](13-gtm-growth.md)

| 이름 | 의미 / 브랜드 적합성 | 기억·발음 | 확인할 위험 |
|---|---|---|---|
| Liar Sweeper | 핵심 숫자 거짓말을 직접 전달 | 기존 지뢰찾기 단어와 연결 | Liar의 부정적 어감, 상표·도메인 |
| Bluffield | bluff + field, 심리전 | 짧으나 철자 설명 필요 | 동일 브랜드·발음 혼동 |
| Mine Mind | 논리·두뇌 경쟁 | 반복음이 쉬움 | 일반 단어라 검색 구분 어려움 |
| Doubt Grid | 숫자를 의심하는 격자 | 기능 전달, 다소 추상적 | 비게임 제품과 충돌 가능 |
| Tell & Sweep | 거짓을 알아채고 판을 푼다 | 리듬과 행동 중심 | 기호·tell 의미 번역 난도 |

2026-11-16 사용자가 앱 이름을 **Liar Minesweeper**로 변경하도록 명시했다. `Minesweeper`를 사용해 지뢰찾기라는 장르를 이름에서 바로 알 수 있게 한다. 위 표는 이전 후보 검토 이력이다. 저장소 이름은 `search-mine`이며 도메인·상표 확보를 확인하지 않았다.

## 결정 사항

공개 표시명은 8언어에서 **Liar Minesweeper**로 통일한다. 로고/접근성 이름·문서 title/검색 메타·PWA name/short_name·데일리 공유·계정/설치 안내와 OAuth 등록 안내에 적용한다.

기존 manifest id/start_url/scope와 저장소/캐시/쿠키/DB/Compose/service/package 식별자는 유지한다. 이름 변경으로 설치나 로컬 기록이 새 앱처럼 분리되지 않게 한다. 보존 원문과 과거 검증 기록의 이름은 당시 이력으로 남긴다.

이 변경은 사용자에게 보이는 이름의 수정이며 게임·인증·개인정보·광고 규칙과 기존 승인 상태를 바꾸지 않는다. [#237 검증](../verification/237-liar-minesweeper-name.md)·FR11/TS18/19/20/26/30/36을 따른다.

## 열린 질문

최종 도메인과 실제 OAuth/앱 설치 표시를 운영자가 확인한다. 제공자 개발자 콘솔의 앱 표시명도 Liar Minesweeper로 등록한다.
