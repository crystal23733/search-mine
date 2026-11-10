# #221 선택적 PWA 설치·파일 없는 결과 화면

FR01/09/10/11/16 · NFR01/02/03/05/06/07/08 · TS17/18/19/20/24/26/30/36 → [ADR0101](../adr/0101-web-workflows-and-installable-pwa.md). 선행 #219/PR220 develop 병합·종료.

## 행동 시나리오

1. 설치 이벤트 전에는 수동 메뉴 안내, 보존한 이벤트 후에는 명시 버튼. prompt는 사용자 클릭에서 동기1회이며 중복 클릭/같은 이벤트 재전달로 재사용하지 않는다. 취소/예외 뒤 웹 이용은 유지하고 새 이벤트로 다시 요청할 수 있다.
2. accepted 결과는 요청 완료로만, appinstalled 관측과 standalone/Apple standalone은 각각 설치 알림/현재 앱 창 실행으로 표시한다. 새 이벤트·모드 변경·dispose 후 늦은 promise가 상태를 덮지 않고 모든 구독/리스너를 해제한다.
3. 8언어 설정 화면의 카드/안내/취소·오류/앱창 상태와 PC/mobile 긴 번역·키보드·일반 웹 동작을 확인한다.
4. production/HTTPS/실제 nginx의 manifest와192/512/maskable/Apple 아이콘이 실제 MIME/정상 바이트로 제공되고8언어 공개 문서가 연결한다. start_url/id/scope에 개인정보/query 없이 기존 언어 선택으로 시작한다.
5. local/official daily 결과와 내정보·두 순위를 웹에서 확인하고 PNG 버튼/다운로드0, 복사·텍스트 공유/취소 뒤에도 결과를 유지한다. 공식 순위는 서버 연결이 필요하다.
6. 기존 안전 업데이트/캐시된 연습/인증·API 비캐시/전체CI를 유지한다. 합성 설치 이벤트는 UI 계약이며 OS 설치 완료가 아니다. #83과 실제 PC/Android/iOS 설치·OAuth 복귀는 별도 검수다.

## 실행 기록

시나리오를 먼저 기록했다. InstallPort stub은 실제6실패/1통과→7통과, 기존 share adapter의 download 잔존은1실패/2통과→Green을 확인했다. 첫 share fixture의 DOM 미주입 오류를 바로잡아 행동 실패와 구분했다. UI 테스트는 promise 완료를 기다리도록 보정했으며 제품 기한/기존 retry는 바꾸지 않았다.

최종 웹 단위460통과/전체줄91.86%, 타입·lint/접근성·형식·build·immutable install 통과. 부분 단위 실행의 전체 coverage 미달은 전체 검사로 대체하지 않고 원문을 보존했으며 최종 전체80 gate를 통과했다. 문서268/Mermaid34/diff 검사 통과다.

로컬 production Chromium PC/mobile14개(12.6초), 고유 합성PG의 실제 HTTPS 공식데일리/두순위/PWA6개(49.8초)가 통과했다. manifest/아이콘 실제 MIME·PNG치수·32개 공개문서·Chromium manifest 해석과8언어 keyboard-trigger/cancel/다운로드0을 확인했다. 최초 sandbox EPERM 포트 차단은 원문을 보존하고 로컬 실행 권한으로 재실행했다.

실제 Docker/nginx smoke는 manifest/icons/CSP,32문서/비밀 실패차단·내부망/권한·403404/cache/API·JS off/WASM·인증WS/종료3.109초·소유 fixture 정리를 통과했다. 고유 합성PG/익명볼륨/URL 파일 제거도 확인했다. source·web 이미지ID와 summary/log는 로컬 리뷰에 보존했다.

전체 CI 결과는 PR에서 정확한 head와 함께 확인한다. 위 합성 이벤트·localhost HTTPS는 실제 OS 설치/실계정 OAuth/확정 도메인·기기 검수의 완료 증거가 아니다. [실기기 절차](../workflow/pwa-installation.md)와 기존 #83은 별도 미완료다.
