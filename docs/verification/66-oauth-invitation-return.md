# #66 검증: OAuth 친구 초대 복귀

2026-10-09 · M3/#17 · FR01/07/08/11, NFR02 · TS01/12/13/19/20/31~36 · [ADR0031](../adr/0031-oauth-invitation-return.md). 선행 #64/PR65 developf8367c7 병합·종료 뒤 진행했다.

## 구현과 실제 Red→Green

로그인 전 canonical8문자 code를 `invite_code`로 전달한다. 서버의 FriendsInvite 목적지가 code와 friends 경로를 결합하고 인증과 로비가 공유 RoomCode 검증을 사용한다. 새 nullable migration은 login/friends·정확8byte/허용 문자를 검사한다. 기존5분 일회용 거래의 insert/원자 consume과 만료 청소에만 저장하며 계정/세션에 추가 정보를 만들지 않는다.

success는 해당 거래의 locale/code로 friends 또는 onboarding에 돌아가고, 소비된 취소/교환 실패는 안전한 login retry로 복원한다. nickname skip/tutorial와 언어 변경은 code를 유지한다. 기존 세션·provider·브라우저/state 바인딩은 유지한다. 로그인 입력은 Rust AuthLoginRequest→생성 TS이며 OAuth credential/state/token·초대를 브라우저 저장소에 쓰지 않는다. provider authorize URL에 초대를 넣지 않고 문서와 auth 응답의 no-referrer를 적용한다.

서버 HTTP의 actualRed는10passed/2failed(400!=200)→12Green이다. 8locale×닉네임 유무16흐름, 다른 browser·중첩3거래/역순·성공/취소/교환 실패·재전송·invalid code/path/duplicate·거래 미할당을 확인했다. 리뷰의 callback query에 다른 invite를 주입하는 probe까지13Green(0.03s)이다. auth service7와 공유 lobby policy14를 검사하고 domain/서비스의 link·reauth 거절과 protocol의 optional/closed fields도 추가했다.

웹 로그인 전송 actualRed1failed 뒤 관련25Green이며 추가 언어 변경 시험을 포함해 전체92Green, lines86.93%(1444/1661)을 확인했다. 최초 전체 실행에서는 Rust 컴파일과 겹친 기존 localpractice 시험이5초timeout으로 실패했다. compiler 종료 뒤 전체 웹을 다시 실행하여92개 통과했다. 최초 디스크 no-space/fixture 이름 충돌의 compile 실패는 제품 Red로 계산하지 않고 생성 산출물 정리/fixture 수정을 분리했다.

기존 two-tab update는 idle 버튼 관측을 보강해도6개 중1개가 실패했다. 임시 추적에서 idle ACK2개·동일 client 집합·2.7/6.6ms 확인·waiting installed/old activated 뒤 skipWaiting Promise가 끝나지 않았고16개 중2개가 reload에 실패했다. message lifetime 분리 후보는 추적을 켜면16개 통과했으나 추적 제거 뒤24개 중4개가 실패하여 폐기했다. 제품 SW/adapter/coordinator는 원본을 복원했고 임시 추적을 제거했다. 기존 시험의 쿼리 URL 변경 대신 ADR0018대로 고정 worker URL의 새 bytes를 자체 loopback 서버에서 제공한다. per-test 임의 포트/메모리 revision·실제 production 산출물·자동 close로 공유 파일이나 사용자 서버를 수정하지 않고 두 탭의 원래 controller·idle 버튼과 실제 reload를 관측한다. timeout 증가나 검사 제외는 없고 안정성은 반복/전체 검사로 확인한다. 브라우저 내부 원인을 확정한 것으로 표현하지 않는다.

## 실제 DB/브라우저와 남은 출구

고정 URL의 기본 headless-shell도16개 중3개 실패했다. 별도 임시 계측에서는 이전 worker의 JS waitUntil pending0을 관측했고 이 값은 Chromium native no-work/renderer-idle과 다르다. 계측을 제거하고 Playwright `channel: chromium`으로 일반 Chromium의 새 headless를 사용하여 PC/mobile 각8개, 총16개를1.7분에 통과했다. actual busy 차단·동일 progress·두 boots2·scoped cache 삭제/재활성·API cache0을 모두 유지했다. 라이브러리/브라우저 버전 pin·timeout 증가·retry 추가·검사 제외는 없다. shell 실패 원본과 실제 결과를 로컬 review에 남기며 전체/CI 검사를 추가 확인한다.

26번째 PostgreSQL 시험은 nullable 호환·DB CHECK·다른 browser/provider·동시 두 consume의1회 성공·canonical code/locale 복원·정확5분 expiry/cleanup·계정0을 검사한다. 로컬26ignored는 실DB 성공으로 계산하지 않는다.

최종 scripts/check.ps1 exit0: locked fmt/Clippy/workspace native/WASM·생성 타입/fixture/tokens·웹 형식/lint/typecheck/92unit(16.15s)/build·DB 없는 browser50(1.6m)·docs119/Mermaid34. 제품 SW/adapter/coordinator diff0과 임시 추적 제거·ignored review를 확인했다. 실제 DB/HTTPS는 CI의 별도 출구다.

HTTPS 새8locale별 시험과 취소 retry1개는 실제 SQL/router/cookies를 사용한다. 네 제공자 proof만 test port로 대체하고 mobile/PC·nickname/tutorial·Storage get/set 거부·provider 요청의 code/return_path/referrer 비노출을 검사한다. 전체 브라우저 출구는 기존57+새9=66개다. 최신 CI의 실제 DB26·HTTPS66·coverage 수치는 PR 및 ignored 로컬 review에 기록한다.

PR67/head09a63642의 CI에서 PostgreSQL26개/ignored0·웹92unit/HTTPS포함66browser(3.3m, 실패/retry0)가 통과했다. Auth385/387=99.48%·Match276/278=99.28%·Lobby362/368=98.37%·전체5710/6165=92.62%·core95 gate 및 이름별 최신6checks를 확인한 뒤2026-10-09T02:04:49Z에 squash 병합했다. developf5f9dd26·#66 CLOSED02:04:51Z를 실제 조회했다.

pm-skills 정확성·보안 review scope66/basef8367c7에서 UI/typed input→same RoomCode/목적지→5분 SQL row/원자 consume→provider proof→실제 nickname/본인 거래 redirect→tutorial의 경계를 순차 조사했다. callback의 다른 proposal을 거절하고 세 거래를 역순 처리하는 forced authority/identity probe를 실행했다. SQL은 bound value이고 redirect는 닫힌 enum/canonical ASCII/allowlisted locale에서만 만들며 실패도 matching transaction을 소비한다. 검토 범위에서 추가로 뒷받침되는 결함은 남지 않았다.

실제 친구방 입장·큐·온라인 게임 화면/클라이언트 재접속은 다음 #17/#18 출구다. URL 복귀만으로 온라인 UI 완료를 주장하지 않는다. 실제 제공자 키/등록 #42·미니 PC #26·사람 #27은 별도 검수다.
