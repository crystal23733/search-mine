# ADR0053 · 본인 게임기록 내보내기와 응답 권위

- 상태: 채택 · 2026-10-11 · M3 #114 / 상위 #19
- 선행: #112/PR113 develop311ab66 병합·112 CLOSED 확인
- 추적: FR01/09/10/16 · NFR01/02/05/07 · TS17/18/20/29/31/34/35/36

## 범위와 소유권

기존 POST /api/v1/me/export와 AuthExport는 계정/연결 정보만 내보낸다. 새 privacy domain과 GameExportReader port/PG adapter는 본인 온라인 결과·데일리 첫 완료와 살아 있는 데일리 raw replay를 제공한다. 두 출처를 기존 설정의 명시적 내보내기 동작에서 모아 하나의 버전 있는 PersonalDataExport 파일로 내려받는다. design_layout/13 설정·접근성.png의 내 데이터 내려받기를 따르며 추가 profile 권한·필드/새 개인정보 수집은 없다. 무계정 로컬 연습/IndexedDB 대기 기록을 서버 계정에 자동 귀속하지 않는다. 공식 데일리 수집/순위/time ticket 활성화는 다음 #19 하위다.

닉네임 없는 계정도 권리를 행사한다. 세션이 현재 유효하고 최근300초 이내 재인증돼야 한다. PG는 기존 authorize_account_transaction의 account→session 잠금과 fresh 정책을 공유한다. 잠금 대기 뒤 trusted clock을 다시 읽고 변경되면 같은 잠금 안에서 권한/시각을 재검사한다. 삭제/철회는 현재 FK·동일 account/session 잠금 순서를 따르며 읽기가 계정을 만들거나 삭제 데이터를 복원하지 않는다.

## 고정 HTTP와 페이지

POST /api/v1/me/games/export는 v1·expected account UUID·public session revision UUID·stream(online/daily)·선택 after UUID를 받는다. POST /api/v1/me/games/replay는 같은 소유권 proof와 completion UUID를 받는다. canonical non-nil UUID, 버전/필드 allowlist·4KiB body/Content-Type·query 금지·Origin/CSRF를 검사한다. account/session/cursor를 URL에 넣지 않는다. 응답은 no-store/no-referrer/nosniff이며 비활성 구성에서도503/no-store로 닫는다.

각 stream을 immutable record UUID ASC로 최대50개+다음 여부1개 읽는다. cursor는 마지막 실제 반환 UUID이고 다음 cursor는 반드시 전진한다. client가 after를 바꾸더라도 SQL은 현재 본인·보존 창을 다시 적용하므로 다른 사람 기록/만료 기록을 읽을 수 없다. ID 순서는 export 순회용이며 완료 시간 순위가 아니다. online 동일 계정의 중복 human seat·손상 projection은 fail-closed다. 페이지 response는 최대64KiB로 제한한다.

각 페이지는 실제 읽기 transaction의 상태다. 여러 HTTP 요청을 하나의 DB snapshot이라고 주장하지 않는다. 내보내는 중 새 기록은 다음 내보내기에 반영될 수 있고, 삭제/만료된 raw replay는 null이 될 수 있다. 모든 페이지/개별 raw 읽기는 현재 세션/권한과 보존을 다시 검사한다. 명시적 실패를 빈 목록/완성 파일로 바꾸지 않는다.

## 최소 데이터와 보존

online은 기존 admitted/legacy retention anchor와 recorded_at을 검증하며 정확90일 경계에서 제외한다. 기존 StoredPersonalResult의 본인 최소 projection을 공유한다. match UUID·rules hash·own stats/result·기록/보존 anchor만 제공하고 hidden seed/board·상대 통계·닉네임 snapshot을 선택하지 않는다. daily는 ADR0052의 실제 accepted definition/receipt·7일/90일 anchor와 strict row decoder를 공유한다. metadata·completion/attempt UUID·본인 진행/실수·비신뢰 개인 replay elapsed·완료 시각·현재 raw 사용 가능 여부만 제공한다. 이 정보를 verified_time으로 표시하지 않는다.

raw는 단일 본인 완료별로 읽으며 completed_at에서 정확7일·90일과 SQL NULL을 검사한다. 물리적으로 아직 남은 만료 replay도 SELECT에서 본문을 읽지 않는다. 반환 전에 기존 strict replay decoder와 canonical SHA256/전체 metadata를 실제 저장 digest/accepted receipt와 비교한다. worker 밖의 다른 입력과 결합하지 않는다. 이 검사는 입력 보존의 일관성을 확인하며 사람이 풀었다는 증명이나 새 공식 제출을 만들지 않는다. digest·subject·credential·session token/hash는 응답에 넣지 않는다.

## 취소와 응답 시점

privacy 전용 AuthorityRegistry와 기존 bounded read rate 정책을 사용하며 온라인/로비 소유권을 교체하지 않는다. 기본 동시 요청4(설정1..64), authority64(설정1..20000), session/account 초당20회·4096-key/60초 bounded eviction이다. rate 알고리즘은 기존 personal result reader와 공유하며 counters/authority는 기능별로 분리한다. global semaphore는 pool acquire·SQL·CPU·post-proof/response 준비까지 포함한다. 전체 요청은2초이며 timeout/cancel 뒤 성공 응답을 만들지 않는다.

raw JSON/metadata/digest 검증과 큰 응답 준비는 동시2 blocking worker/2초 기한으로 제한한다. 실제 worker가 permit을 소유하여 caller timeout/cancel이 실행 중 CPU의 자리를 먼저 돌려주지 않는다. 기존 데일리의 같은 수명 계약을 공유하거나 재사용 가능한 executor로 추출하며 기존 daily API/수치/동작은 유지한다. 전후 실제 session proof에서 expected account/revision·freshness·clock 역행을 검사하고 기존 invalidation barrier로 마지막 순수 response commit과 철회를 선형화한다. 네트워크로 이미 보낸 바이트를 되돌릴 수 있다는 보장은 하지 않는다.

## 브라우저와 완료 조건

Rust DTO를 TS로 생성한다. AuthPort의 기존 명시 export 동작에서 계정 정보→online/daily 페이지→사용 가능한 raw를 같은 account/session revision으로 수집한다. 기존 nickname 없는 operate 권한을 유지하며 gameplay 전용 AuthenticatedRequests의 nickname 조건으로 권리를 막지 않는다. 각 응답의 v/account/stream/record/cursor와 raw completion/metadata를 strict 검사하고 마지막 fresh bootstrap·현재 generation/세션을 확인한다. 실패/취소/계정 전환/늦은 응답은 다운로드하지 않는다. replay가 정상 만료/NULL이면 null로 반영하며 오류를 null로 바꾸지 않는다.

collector는 최대2048페이지/32MiB aggregate로 제한하고 상한 초과·반복/역행 cursor·malformed 응답은 완성 파일을 만들지 않는다. 서버의 bounded 페이지 API는 계속 사용할 수 있으며 브라우저 한 파일을 무제한 메모리로 만든다고 약속하지 않는다. credential/세션은 파일·localStorage/IndexedDB·URL에 저장하지 않는다. 각 HTTP 전송의10초 제한은 유지하며 최근 인증 만료는 재인증 후 다시 내보내도록 기존8언어 feedback을 사용한다. 계정 생성/로그인·대기 제출을 자동 시작하지 않는다.

시나리오→실제 Red→Green→Refactor, 실제PG pagination/본인·타인·삭제/철회·exact7일/90일/clock/DST/손상·SQL lock/실패, HTTP 전후 proof/invalidations/limits/CPU cancellation, 실제 HTTPS PC/mobile download·8언어/키 없는 실행을 검사한다. Privacy domain95%·기존 도메인 gate/전체 서버80%·full check·fresh coverage/docs/Mermaid·correctness 리뷰/최신CI→develop squash merge→114 CLOSED 뒤 공식 제출 하위를 진행한다. physical cleanup/backup25·미해결83·외부42/26/27은 별도다.

## 검토 보완 · 2026-10-11

clock은 요청 시작뿐 아니라 reader 완료 시각부터 마지막 proof까지도 역행을 거절한다. 브라우저 collector는 정상적인 여러 페이지가 자체20/s 제한을 반복해서 넘기지 않도록 요청 사이 최소100ms를 두며 대기는 abort 가능하다. 응답 오류를 자동 재시도하지 않는다. 완성 파일의 실제 다운로드는 AuthPort.exportTo의 현재 generation 확인 직후 같은 JavaScript 실행 구간에서 sink를 호출한다. 파일 DTO에는 이 메모리 권위나 credential을 넣지 않는다.
