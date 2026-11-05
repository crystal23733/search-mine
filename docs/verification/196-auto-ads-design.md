# #196 · 자동 광고 설계 전환

[ADR0091](../adr/0091-adsense-auto-ads.md) · FR12/13·NFR01/02/05/07/08·TS18/20/23/24/25/30.

2026-11-05 사용자 자동 광고/교체 가능한 script 지시를 승인 근거에 기록했다. 선행194/PR195 MERGED/CLOSED·develop5fd90eb clean/origin0:0을 확인했다. 자동 배치·현재 목적/공개 문서·기본 미설정0·게임 문서 격리·실제 게시자ID/운영147을 정리했다. 자체3판 cap/수동전면/H5 기본 계획을 대체하고194의 미사용 정책 제거를 후속 제품 이슈로 정의했다. 기존 원문/검증 이력은 보존한다.

설계 리뷰: script 노드 삭제는 SDK unload가 아니므로 시작 전 document 표시와 router publish 전 새문서 barrier를 사용한다. 보호 문서·query/hash·첫방문 tutorial을 제외하고 철회/계정/오프라인/BFCache도 재검사한다. Google의 자동 배치/빈도를 직접 복제하지 않고 script load를 impression으로 세지 않는다. 실제 설정은 publisher ID/사이트 승인/CMP이며 API키가 아니다. 실제 overlay 비활성·page exclusions·CSP/ads.txt/SDK 검수는 운영 입력으로 남긴다. 재설계 범위는 최신 사용자 지시로 승인되었고 기존 병합 위임을 유지한다.

이 PR은 문서만 변경하며 제품 코드·TDD/브라우저·실광고 검수를 수행한 것으로 보고하지 않는다. 문서/Mermaid·필수checks·실제 리뷰/정확head·병합/CLOSED는 PR과 당일 로컬 review에 수행 후 기록한다.
