# #194 · Rust 광고 요청 정책

[ADR0090](../adr/0090-ad-request-policy.md) · FR12/13·NFR01/02/05/07/08·TS23/24/25.

선행190/PR191·192/PR193 MERGED/CLOSED, 최신develop9e88bf4 clean/origin0:0 확인. 제품 코드 전에 설계/시나리오를 기록했다. 실행한 검증과 실제 코드 리뷰 결과는 수행 후 기록한다. 실제 CMP/SDK·원자 저장/다중 탭·화면 hook과 상위22/21/147·83은 별도 미완료다.

## 구현·실행 검증

Rust core는 화면/placement·현재 연결/광고목적/SDK·exclusive slot의 관측값과 기기/계정의 각 완료 번호를 검사한다. 결과 예약 계획은 양쪽 마지막 요청을 각자의 현재 완료로 이동시키며 SDK 실패 뒤 되돌리는 API는 제공하지 않는다. 이는 adapter가 먼저 저장할 계획이며 실제 저장·잠금 구현을 대체하지 않는다. menu 배너와 결과 전면의 계획을 구분한다.

- 실제 행동 Red: 차단만 반환하는 최소 진입점에서 첫3/이후6·양쪽 제한·메뉴의 기대 결과3개가 실패했다(.tmp/ads194-red.log). 구현 뒤 core8개와 protocol3개 통과. 임의 유효 카운터의 gap/예약 후 재요청 차단 property도 포함한다.
- 폐쇄 입력은2048byte 경계·알 수 없는/누락/중복 필드·enum·version·음수/분수/u32 초과를 검사한다. canonical comparison이 nullable 누락과 enum extra까지 검증하며 generated TS의 discriminant도 유지한다. 기존 ts-rs의 deny_unknown_fields 경고1개는 이전 검증과 같고 새 enum은 기존 방식의 decoder 비교로 엄격함을 유지한다.
- native generator의 광고46case를 별도 fixture로 보존하고 기존 fixtures:generate/check에서 함께 관리한다. 실제 PC/mobile WASM의 같은 정책/decoder 결과와 모두 일치했다(Playwright2PASS3.6초). SDK/실광고/거짓 동의 endpoint는 없다.
- 웹 전체402PASS, 라인91.23%(3704/4060). lint/typecheck/format·production build·WASM build/target check·clippy(-D warnings)·문서239개/Mermaid35개 통과. 필드 주석 추가 뒤 생성 TS stale 검사를 발견해 원천에서 다시 생성했으며 최종 생성 타입/WASM 선언/fixture check도 통과했다. ts-rs가 필드 doc 주석에 만든 trailing whitespace는 필드 설명을 일반 코드 주석으로 두어 원천에서 해결했다.
- 로컬 전체 Rust의 첫 실행은 기존 lobby_http의 실제 loopback bind가 sandbox PermissionDenied로 실패했다. 동일 전체 명령을 loopback 허용 환경에서 다시 실행해475PASS/실PG185ignored를 확인했다. 로컬 실PG/LLVM/전체 PowerShell/전체 브라우저는 이번에 미실행이며 CI의 실제 회귀/coverage 결과와 구분한다.

## 실제 코드 리뷰

빈도 차감 전에 기기/계정 counter의 범위를 모두 검사해 역전/overflow를 차단한다. 두 완료 번호는 서로 같을 필요가 없으며 각각 gap을 만족해야 하므로 단순 max를 쓰지 않는다. 예약 뒤 원자 저장과 판별 marker가 필요하다는 조건을 core/WASM 문서·ADR·생성 API에 명시했다. 현재 bool·guest/account 구분과 카운터는 adapter의 관측 입력일 뿐 서버 권한 증거가 아니다.

TS에 빈도/게임 규칙을 복제하지 않고 실제 WASM 호출은 protocol의 동일 decoder/정책을 사용한다. native 기대값은 core에서 생성해 decoder와 독립 비교한다. 최대 입력 크기를 parsing 전에 검사하며 DTO에는 계정/세션/seed/URL·선택 proof가 없다. 기존 native 게임 fixture는 변경하지 않고 광고 fixture helper만 분리했다.

실행 앱/SDK·데이터 수집·스토리지/DB·SW·브라우저 flags/timeout/retry·의존성은 변경하지 않았다. 실제 완료 집계/원자 저장/다중 탭 잠금·화면/광고 callback/노출 종료 barrier/timeout·SDK/CSP/운영147은 상위22 후속이다. 이 이슈로 상위22/21/147·83을 종료하지 않는다. 필수 CI/정확 head·최종 리뷰/병합은 PR과 당일 로컬 review에 기록한다.
