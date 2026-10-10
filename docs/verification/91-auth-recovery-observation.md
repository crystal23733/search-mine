# #91 인증 철회 복구의 응답 관측 검증

M3 #18 · FR14/NFR05/08 · TS15/16/20/26/31/35/36 · [ADR0041](../adr/0041-auth-recovery-response-observation.md). 선행89/PR90 develop c36a4fe 병합·종료 후 시작했다.

## 실행 근거

Red: Product38023948678 auth-recovery390px에서 실제 logout 후 bootstrap Response.json이 CDP No data found로 최초 실패·재시도 성공. 실패 trace는 .tmp/result89-ci-failures에 보존했다. 마지막 page fetch200의 body resource 부재를 확인했고 새 navigation이 없으므로 도구 메시지를 실제 navigation 원인으로 단정하지 않는다. null proof 이후 production recovery의 signal 취소와 별도 CDP read의 수명 경계를 소스로 확인했다. Chromium 내부 원인/제품 결함의 확정 재현이라고 주장하지 않는다.

수정 전 로컬 실제 PG17.4/HTTPS PC/mobile20회29.7초·재시도0 통과로 간헐 실패를 로컬에서 재현하지 못했다. 변경 후 결과·전체 검증·CI·PR/병합은 실제 실행 후 기록한다. 재시도 성공으로 최초 Red를 없애지 않고 assertion/timeout/retry를 낮추지 않는다.

변경 후 같은 실제 PG17.4/HTTPS20회18.3초·재시도0 통과. one-use route.fetch는 실제 status200/no-store/account:null/session_revision:null 본문을 확인하고 같은 response를 전달한다. 그 후 실제 앱의 오류 안내·download0와 독립 bootstrap:null·쓰기0도 통과했다. 최신 production build 뒤 전체88browser2.4분·재시도0 통과, format/lint/typecheck·docs141/Mermaid34실패0. 초기 포맷 검사만 실패해 해당 파일을 Prettier로 정리하고 전부 재실행했다. Rust/웹 제품과 의존성/lockfile/migration은 변경하지 않았다. 이전 이슈의 전체 unit/서버 coverage를 이번 로컬 재실행이라고 보고하지 않는다.

correctness 리뷰는 develop c36a4fe 대비 테스트 변경과 production SessionRecovery→signal 폐기·실제 HTTP→route.fetch/APIResponse→동일 fulfill→권한 UI/독립 proof 흐름을 순차 확인했다. 동시 Promise.all이 route 오류를 수신하고 one-use·redirect0/retry0/기존10초로 서버 실패를 숨기지 않는지, 임의 JSON/다른 세션 응답/일반 storage를 만들지 않는지 확인했다. 기존 행동 단언이 유지되고 검토 범위에서 지원되는 미해결 결함은 없다. Chromium 내부 원인·보안/성능 전체 감사는 확정하지 않는다. CI·PR·병합은 원격 실행 후 기록한다.

## 범위와 남은 일

실제 backend response 관측/전달과 기존 권한 행동 단언을 함께 유지하는 테스트 경계다. #83 두 탭 load 원인·영속 journal/startup abort·미확인 통계·새로고침 결과 발견·외부 OAuth42/장비26/사람27은 미해결 별도 출구다.

## 원격 완료

[PR92](https://github.com/crystal23733/search-mine/pull/92)은 2026-10-10T04:52:32Z develop8347efaf764b4bb221770595c9e6308543a93f43에 병합됐고 #91은04:52:33Z 종료됐다. Product38025093554·최신 Repository38025516857 포함 전체 checks SUCCESS/Ready/CLEAN/head812ce332985cb13b7582b0a3b3487761eb17070c 확인. CI 실제 PG18.6 DB30/ignored0(6.30초/coverage7.80초),197unit35파일11.54초·웹87.89%·서버92.77%,88browser4.1분·재시도0. #83 원인 미확정은 종료하지 않는다.
