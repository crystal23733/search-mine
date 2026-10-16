# #152 · 목적별 동의 포트와 설정 화면

FR11/13 · NFR01/02/06/07/08 · TS19/24/26/29/31/36. 선행20/150 CLOSED·PR146/151 병합과 최신 develop9eb6a91/clean을 확인했다. [ADR0071](../adr/0071-purpose-consent-port.md)을 제품 코드보다 먼저 작성했다.

인수 경계는 닫힌 목적별 default-deny·매 조회 current snapshot·명시 재열기/취소/실패/late completion·구독/요청 정리,8언어 설정의 실제 DOM과 실행 앱의 unavailable·무광고/연습 무차단이다. 실제 CMP/계정·탭·서버 동의/선택 이벤트는 후속이며 현재 runtime은 미설정뿐이다.

실제 목적별 권한 Red0pass/1fail(analytics 허용 부재)와 수명/예외 확장 Red9pass/7fail/1.59초를 확인했다. current snapshot의 닫힌 decoder·목적 독립·재열기/취소/구독/dispose 수명을 구현한 Green16pass/1.53초를 확인했다. 실제 설정 DOM의 재열기 버튼 부재 Red0/1/2.67초 후 포트를 연결했고,8언어/실패/취소/이탈 확장에서 화면 제거 직후 abort=false인 Red28pass/1fail/2.78초를 관찰했다. 설치된 Preact11의 passive unmount cleanup이 after-paint로 지연되는 근거를 확인하고, ADR에 같은 commit의 layout cleanup을 먼저 반영한 뒤 Green29pass/2.99초를 확인했다. 늦은 이전 completion이 새 pending을 끝내지 않는 경계는 microtask 완료 후에도 검사했다.

웹 전체305개/11.54초·라인91.2%, 새 consent 서비스69/69=100%·ConsentSettings21/22=95.45%를 실제 측정했다. typecheck·lint·format과 실제 Chromium PC/mobile4개/11.4초(8언어1440/390/360px·unavailable/선택 요청0·저장소 차단/실제503/로컬WASM)를 통과했다. lint에서 발견한 output의 중복 role은 native implicit status로 정리했다.

정확성 리뷰 기준 develop9eb6a91: adapter 현재 선택→엄격한 최소 view·목적별 허용, CMP 변경→구독/조회, pending/abort/dispose/이전 완료→현재 요청과 UI 수명을 대조했다. 취소 자체는 동의를 만들지 않고 이후에도 현재 선택만 읽는다. 실제 runtime은 createConsent() 미설정뿐이며 fake CMP는 테스트 안에서만 조립한다. raw payload/선택 저장/네트워크/게임 lock·server grant를 새로 추가하지 않았다. 실제 CMP·계정탭·서버 선택 저장은 후속 범위다.

새 독립 DB 전체·문서/Mermaid·최신CI와 병합은 수행 뒤 PR/당일 로컬 review에 구분해 기록한다. #21/#147/M4·#83/#141은 OPEN이다.
