# #150 · 실제 저장소·쿠키 고지

FR01/10/11/13 · NFR01/02/06/07/08 · TS18/19/24/26/31/35/36. 선행20/148 CLOSED·PR146/149 병합과 최신 develop2c4a62d·clean을 확인했다. [ADR0070](../adr/0070-technical-storage-notice.md)을 제품 코드보다 먼저 작성한다.

인수 경계:8언어 실제 저장 범주/유지·삭제, 모든 화면 footer·언어별 route와 설정 dialog의 공통 읽기 전용 내용, 읽기/닫기 무부작용, 저장소 차단/서버 실패·PC/mobile·keyboard/초점·로컬 연습 무차단이다. 실제 쿠키/설정/로컬 기록/캐시 코드와 문구를 대조하고, 공개 운영자나 동의/광고 성공을 꾸미지 않는다.

실제 App DOM Red0pass/1fail/3.03초는 footer 저장소 링크 부재였다. 공통 route/organism/dialog를 연결한 뒤 Green1pass/2.84초,8언어 확장9pass/3.42초를 확인했다. 리뷰에서 공식 데일리의 시작 확인 후 자동 제출과 로컬 연습의 명시 대기 제출을 구분하도록 보완했고, 누락 문구의 실제 Red8pass/1fail/3.41초를 확인했다.

실제 Chromium에서 긴 dialog가 하단 닫기 초점으로 스크롤되어 첫 고지 viewport ratio0인 Red1fail/5.6초를 관찰했다. 실패 trace/context는 로컬 .tmp/storage150-focus-red에 보존했다. ADR에 초기 제목 초점→Tab 닫기/스크롤→Escape/opener 복구를 먼저 반영하고 기존 Dialog의 선택 focusTitle과 고지에 적용했다.8언어/PC1440·mobile390·360px/초점 복구/저장소 차단·실제 실패 bootstrap/로컬 WASM 연습의 브라우저4개/7.7초가 통과했다. 최종 전체와 강화한 네트워크 assertion은 다음 검증에서 확인한다.

문구와 preferences/learning/offline/daily-records/worker/auth cookie/session·공식 시작 confirmation을 대조했다. 고지는 정적/읽기 전용이며 auth refresh 등 기존 앱 동작 외 별도 요청/동의 grant/저장소 열거가 없다. 실제 CMP/법적 정책 완료나 외국어 사람 검수로 해석하지 않는다. 전체·최신CI와 병합 결과는 수행 뒤 PR과 당일 review에 구분해 기록한다. #21의 실제 CMP/현재 consent·선택 수집/전체 정책, #147 외부 검수, #83/#141은 OPEN으로 유지한다.
