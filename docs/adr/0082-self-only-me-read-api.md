# ADR0082 · 내 정보의 본인 조회 API

- 상태: 채택 · #177 · 선행 #176/PR179 병합·CLOSED
- 승인 방향: [ADR0081](0081-my-info-and-export-removal.md), 2026-11-01 사용자 내 정보·파일 다운로드 제거 요청
- FR01/09/11/13/16 · NFR01/02/05/07/08 · TS17/19/20/21/22/24/29/34/35/36

## 계약

`POST /api/v1/me/overview`는 strict `{v:1}`, `/api/v1/me/history`는 `{v:1,kind:"online"|"daily",opponent:"all"|"human"|"bot",cursor?:string}`을 받는다. daily의 opponent는all만 허용한다. body4KiB·JSON MIME 단일·query 금지·같은 Origin/CSRF·현재 세션이 필요하다. 본인은 쿠키에서 결정하며 account ID·임의 기간·limit 입력을 받지 않는다. 일반 조회는 닉네임이나 최근300초 재인증을 요구하지 않는다.

overview는 최소 계정/제공자 연결 시각·현재90일 온라인 요약·온라인/데일리 최근3개·데일리 건수·현재 동의 view를 반환한다. history는 같은 기간/필터 전체 total·온라인 summary(데일리는null)·최대20개/다음cursor를 반환한다. 계정id/관측시각/기간을 응답에 포함해 브라우저가 소유권·필터/기간을 대조할 수 있다. credential/subject digest/세션·raw replay·event·온라인정답·상대식별자는 반환하지 않는다.

기간은 `max(0,observed_at-7776000)` 초보다 큰 원래 retention anchor(데일리는완료시각)다. 저장 결과의 기존 Rust projection과 accepted daily receipt 검증을 재사용한다. 상대 row가 없어 종류를 증명하지 못하면unknown이며모두보기에는포함하되사람/봇필터에서제외한다. summary는Rust가계산하고실제Win/Loss/Draw만played·승률분모에포함한다. abort/cancel·0표본/null을구분한다.

정렬cursor는version/kind/opponent/정렬시각2개/UUID의닫힌JSON을base64url로감싼최대512byte문자열이다. 정렬시각은서버정수microsecond여서소수timestamp동률에서누락하지않는다. 클라이언트는이를opaque문자열로다룬다. canonical·범위/필터일치검사를하며권한으로쓰지않는다. 다른계정cursor나변조cursor여도현재본인SQL필터를벗어나지못한다. 매페이지현재보존·세션을검사하며여러요청의단일snapshot은보장하지않는다.

## 읽기와 자원 경계

전용 MeReader/PG adapter와HTTP권위를둔다. account→session잠금을잡고현재세션revision/hash/시각을확인한다. 같은transaction의계정·기록·현재동의를읽고잠금대기/읽기/반환전시각을다시검사한다. DB 오류·손상·한도 초과를 빈 기록이나 부분 요약으로 바꾸지 않는다. 집계에 기여한 가장 이른 기록/현재 동의의 만료와 세션 만료 중 최소 시각을 응답 유효 기한으로 전달한다. CPU 작업이나 응답 직전에 이 기한을 넘으면 재조회가 필요한 unavailable로 닫아 오래된 요약/동의 허용을 반환하지 않는다.

정확한기존projection검증과기간전체집계를위해조회종류당최대10000개원천행을허용하고10001번째는unavailable이다. 최신20개만합쳐전체전적으로표시하지않는다. 원천조회·CPU·응답을포함해전체2초,요청동시4·CPU동시2·권위64·session/account20/s·4096-key/60초다. CPU작업은실제worker가permit을소유하여timeout/caller취소가자리를먼저돌려주지않는다. 이는개발한도이며미니PC출시성능이나무제한전적조회보장이아니다.

HTTP는전후실제session과취소/철회authority를검사하고응답직전guard로철회와선형화한다. 응답은no-store/no-referrer/nosniff·64KiB상한이다. main/실제HTTPSfixture는동일router/PG구성을사용하고별도invalidator를조합한다. 미설정실행은두route를503/no-store로닫는다.

현재동의는본인session현재행을읽기만한다. 행없음/현재/만료와실CMP미설정을구분하고실제CMP연결은false로표시한다. 만료선택은허용으로표시하지않으며view는at-write권한을발급하지않는다. 새수집/과거동의이력/설정동기화/파일기능을추가하지않는다. 기존파일API는#178에서새화면과함께제거한다.

## 검증

닫힌wire·cursor·기간/summary/종류/null·정렬/20개·권한/계정교체/삭제·검증후철회/역순·DB잠금2초·CPU취소·미설정·HTTP/실PG를행동Red/Green으로확인한다. 새Me domain을95%coverage에포함하고전체80%를유지한다. Rust DTO에서TS를생성하며새UI/파일제거완료는#178의실제HTTPS검증으로확인한다.
