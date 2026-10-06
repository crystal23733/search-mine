# pm-skills 적용 기록

설치된 **pm-skills 2.1.0**의 SKILL.md를 실제로 읽고 각 프레임워크를 아래 문서에 적용했다. 별도 skill 실행 도구가 없는 환경이므로 파일 지침을 읽고 산출물을 작성하는 방식으로 사용했다. 원문의 파일명과 산출물 순서를 유지했다. 문서는 순서대로 앞 결과를 입력으로 사용한다.

| 순서 / 문서 | 적용 스킬과 주요 방식 |
|---|---|
| [01 vision](../product/01-vision.md) | product-vision: 4후보→비전 선택·가치 정렬 |
| [02 competitors](../product/02-competitors.md) | competitor-analysis: 5개 공식 출처·직접/인접·확인/추론 구분 |
| [03 segments](../product/03-segments-personas.md) | market-segments + user-personas: 4 JTBD 세그먼트·3가설 persona |
| [04 journey](../product/04-journey.md) | customer-journey-map: 접점·감정·고통·Aha·이탈 지점 |
| [05 positioning](../product/05-value-positioning.md) | value-proposition + positioning-ideas: 6항목 가치·5문구 비교 |
| [06 canvas](../product/06-lean-canvas.md) | lean-canvas: 9블록·비용/수익 가정 |
| [07 monetization](../product/07-monetization.md) | monetization-strategy: 사용자 광고 전용 조건 내3배치·손익/실험 |
| [08 SWOT](../product/08-swot.md) | swot-analysis: 내/외부 요인·교차 실행/담당/지표 |
| [09 NSM](../product/09-north-star.md) | north-star-metric: Attention game·7기준·입력 지표 |
| [10 assumptions](../product/10-assumptions.md) | identify-assumptions-new + prioritize-assumptions: 8위험·영향×위험 |
| [11 experiments](../product/11-experiments.md) | brainstorm-experiments-new: XYZ·행동 관찰·실패 기준 |
| [12 OST](../product/12-ost.md) | opportunity-solution-tree: 단일 결과→3기회→각3해결→실험 |
| [13 GTM](../product/13-gtm-growth.md) | gtm-strategy + growth-loops: 채널/메시지·5루프·30/60/90일 |
| [14 naming](../product/14-naming.md) | product-name: 5후보·발음/브랜드·도메인 미확인 |
| [15 risks](../product/15-pre-mortem.md) | pre-mortem + strategy-red-team: Tiger/Paper/Elephant·steelman·kill 기준 |
| [16 PRD](../product/16-PRD.md) | create-prd: 8섹션·KR·FR/NFR·범위·릴리스 |
| [17 stories](../product/17-stories.md) | user-stories + job-stories: 3C·INVEST·16US/3JS·AC |
| [18 priority](../product/18-prioritization.md) | prioritization-frameworks: MoSCoW·위험/의존·미측정 RICE 회피 |
| [19 scenarios](../product/19-test-scenarios.md) | test-scenarios: 30목적/전제/역할/행동/결과/경계 |
| [20 metrics](../product/20-metrics.md) | metrics-dashboard: 분모/창/출처/경보·동의 코호트 |
| [21 roadmap](../product/21-roadmap-sprints.md) | outcome-roadmap + sprint-plan: 결과 중심 M0~M6·1인 capacity·20%여유 |

출처 패키지는 `C:/Users/cryst/.codex/plugins/cache/pm-skills/` 아래 pm-product-strategy/pm-market-research/pm-marketing-growth/pm-product-discovery/pm-go-to-market/pm-execution의 `2.1.0/skills/<name>/SKILL.md`다. 이름이 다른 대체 스킬을 쓴 경우는 없다. 사람이 확인할 인터뷰 자료가 없어 persona·시장 크기·수익은 가설/추정으로 명시했다.

프로젝트 스킬은 system `skill-creator` 지침에 따라 `.agents/skills/`에 만들고 Claude용 entry point에서 공유 지침을 참조한다. 불필요한 전역 설정과 외부 계정 변경은 만들지 않는다. 스킬 frontmatter는 bundled quick_validate로 검사한다.
