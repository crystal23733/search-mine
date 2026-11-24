# 21. 시스템 흐름과 데이터 관계

[PRD](../product/16-PRD.md)의 사용자 흐름부터 실제 API, 저장소, 운영까지 연결한다. 각 그림에서 생략한 제약과 예외 처리는 연결된 상세 설계와 검증 문서를 따른다. 다인전은 [통합 검증 #249](../verification/249-multiplayer-integration.md)까지 반영한다.

[전체 구조](#전체-구조와-배포) · [사용자 여정](#사용자-여정) · [OAuth](#oauth-로그인과-계정) · [온라인 대전](#온라인-대전) · [데일리](#데일리와-오프라인) · [동의](#광고와-자체-수집-동의) · [ERD](#postgresql-erd) · [운영](#기동-백업-부하-확인)

## 전체 구조와 배포

현행 배포 설계는 홈 미니 PC를 사용한다. Oracle 서울 VM은 같은 Docker Compose를 옮겨 실행하는 **검토 후보**다. 호스팅 선정과 실제 용량 검수는 [#24](https://github.com/crystal23733/search-mine/issues/24)와 [#26](https://github.com/crystal23733/search-mine/issues/26)에 남는다.

```mermaid
flowchart TD
    Browser["사용자 브라우저"] -->|"HTTPS · WSS"| Edge["Cloudflare DNS · CDN"]
    Edge -->|"연결된 터널로 요청 전달"| Tunnel
    subgraph Host["Docker 호스트 · 현행 홈 미니 PC · 검토 후보 Oracle 서울 VM"]
        Tunnel["cloudflared"] -->|"edge 네트워크 · 8080"| Nginx["Nginx"]
        Nginx -->|"정적 파일 제공"| Files["HTML · JS · CSS · WASM"]
        Nginx -->|"app 내부망 · /api/ · WS · 3000"| Server["Rust · axum"]
        Server -->|"db 내부망 · 5432"| Postgres[("PostgreSQL")]
        Postgres --- Volume[("영속 볼륨")]
        Migration["migration · grant"] -->|"db 내부망"| Postgres
    end
    Tunnel -.->|"호스트에서 터널 연결 시작"| Edge
    Server -->|"외부 HTTPS"| OAuth["Google · Apple · 카카오 · 네이버"]
```

게임·DB 포트는 호스트에 공개하지 않는다. Cloudflare에 캐시된 정적 자산은 엣지에서 응답하고 인증·API·WebSocket은 원본을 거친다. [배포·네트워크](02-deployment-network.md) · [Compose](../../deploy/compose.yml)

### 규칙과 의존성

```mermaid
flowchart LR
    UI["브라우저 UI · Preact / PixiJS"] --> Local["무계정 연습 · 튜토리얼 · 로컬 데일리"]
    UI --> Online["인증 · 온라인 대전 · 공식 데일리"]
    Local --> Wasm["공개 WASM 어댑터"]
    Online --> Wire["같은 출처 HTTP/WS · Rust 원천에서 생성한 TS 타입"]
    Wire --> Runtime["Rust 서버 · 입력과 공개 응답"]
    Wasm --> Core["공유 Rust 규칙 코어"]
    Runtime --> Core
    Runtime --> Ports["계정·결과·시계·외부 제공자 포트"]
    Adapters["PostgreSQL · OAuth · 시스템 시계 어댑터"] -->|"구현"| Ports
```

온라인 비공개 시드와 지뢰 위치는 서버 안에 둔다. 클라이언트는 본인에게 공개된 보드 상태와 상대 요약으로 화면을 그린다. [구조](01-architecture.md) · [규칙](04-game-rules-spec.md) · [프로토콜](06-protocol.md) · [클라이언트 구조](08-client-architecture.md)

## 사용자 여정

```mermaid
flowchart TD
    Visit["웹 또는 설치된 PWA 열기"] --> Locale["지원 언어 선택 · 언어별 URL"]
    Locale --> Home["홈 또는 첫 방문 튜토리얼"]
    Home --> Local["무계정 연습 · 로컬 봇 · 로컬 데일리"]
    Home --> Rank["공식 데일리 순위 열람"]
    Home --> Intent["온라인 대전 · 공식 제출 · 내 정보 · 초대 링크"]
    Intent --> Session{"유효한 로그인 세션?"}
    Session -->|"없음"| Auth["OAuth 로그인"]
    Session -->|"있음"| Continue["요청한 흐름 이어가기"]
    Auth --> Profile["필요한 닉네임·튜토리얼 단계"]
    Profile --> Continue
    Continue --> Match["빠른 대전 · 친구 방 · 공개/초대 다인 방"]
    Continue --> Daily["공식 데일리 도전 또는 대기 기록 제출"]
    Continue --> Account["내 정보 · 계정 · 현재 동의"]
    Match --> Result["대전 결과 · 재대결"]
    Daily --> Receipt["본인 공식 기록 · 순위 · 텍스트 공유"]
    Local --> LocalRecord["기기에서 개인 결과 확인"]
```

[화면·키보드·터치 흐름](09-screens-ux.md) · [8언어 URL](10-i18n.md) · [PWA 설치](../workflow/pwa-installation.md)

## OAuth 로그인과 계정

```mermaid
sequenceDiagram
    actor B as 브라우저
    participant R as Rust 인증 서버
    participant D as PostgreSQL
    participant O as OAuth 제공자
    Note over B,R: 게임 도메인의 요청은 Cloudflare Tunnel과 Nginx 경유
    B->>R: GET /api/v1/auth/bootstrap
    R-->>B: 브라우저 쿠키와 메모리 CSRF
    B->>R: POST /api/v1/auth/{provider}/start
    R->>D: 브라우저에 묶인 일회 로그인 거래 저장
    R-->>B: 제공자 인증 주소
    B->>O: 제공자 화면에서 로그인
    O-->>B: 게임의 콜백 주소로 돌아가도록 응답
    B->>R: GET /api/v1/auth/{provider}/callback
    R->>D: state·브라우저·제공자 검증 후 거래 원자 소비
    R->>O: 코드 교환
    O-->>R: 제공자의 인증 응답
    R->>R: 제공자별 검증과 최소 식별자 추출
    R->>D: 계정 확인 또는 생성 · 서비스 세션 저장
    R-->>B: HttpOnly 세션 쿠키와 복귀 주소
```

계정 연결·해제와 삭제는 본인 인증·최근 재인증을 거친다. Apple 철회용 자격 증명은 암호화해 보관한다. 앱의 실제 제공자 설정과 실계정 확인은 [#42](https://github.com/crystal23733/search-mine/issues/42)에 남는다. [인증·개인정보](17-auth-privacy.md) · [OAuth HTTP 검증](../verification/44-oauth-providers-http.md)

## 온라인 대전

### 1:1 빠른 대전과 친구 방

```mermaid
sequenceDiagram
    actor A as 플레이어 A
    actor B as 플레이어 B
    participant L as Rust 로비
    participant J as 매치 기록
    alt 빠른 대전
        A->>L: v1 로비에 빠른 대전 요청
        alt 서버의 10초 대기 기한 내 상대 예약
            B->>L: 같은 난이도 빠른 대전 요청
        else 대기 기한 도달
            L->>L: 동일 예약을 관측형 봇 대전으로 전이
        end
    else 친구 방
        A->>L: v1 친구 방 생성
        L-->>A: 만료되는 초대 코드
        B->>L: 초대 코드로 명시 입장
        A->>L: 준비
        B->>L: 준비
    end
    L->>L: 슬롯 예약과 같은 배치의 보드 준비
    L->>J: 대전 입장 기록 저장
    L-->>A: 상태 조회에 매치 정보 반환
    A->>L: /api/v1/ws 연결
    opt 사람 상대
        L-->>B: 상태 조회에 매치 정보 반환
        B->>L: /api/v1/ws 연결
    end
    L-->>A: 공개 스냅샷과 경기 진행 상태
```

[빠른 대전 화면·실제 WS](../verification/70-online-quick-match-ui.md) · [친구 방·OAuth 초대 복귀](../verification/72-friends-room-ui.md)

### 2~4인 방과 라운드

아래는 4명이 정상적으로 한 판을 끝내는 경로다. 시작 시 경기 슬롯 하나와 참가자 전원의 연결 자리를 예약한다. 진행 중 여석으로 들어온 멤버는 다음 판을 기다린다.

```mermaid
sequenceDiagram
    actor H as 방장
    participant P as 나머지 참가자
    participant R as Rust 로비·라운드
    participant D as PostgreSQL
    H->>R: POST /api/v2/rooms 방 생성
    P->>R: 공개 목록 또는 초대를 통해 명시 입장
    H->>R: 준비
    P->>R: 각자 준비
    H->>R: 방장 시작 요청
    R->>R: 참가자 검증 · 경기 1슬롯과 연결 4자리 예약 · 보드 준비
    R->>D: 입장 기록 저장
    R-->>H: 상태 조회에 라운드 정보 반환
    R-->>P: 각자의 상태 조회에 라운드 정보 반환
    H->>R: /api/v2/ws 연결
    P->>R: 각자 /api/v2/ws 연결
    R->>D: 전원 연결과 서버 시작 시각 도달 후 시작 기록 저장
    loop 경기 진행
        H->>R: 열기·공격 등 입력
        P->>R: 각자 게임 입력
        R-->>H: 본인에게 공개된 보드와 상대 진행 요약
        R-->>P: 각자에게 공개된 보드와 상대 진행 요약
    end
    R->>R: 첫 완주에서 종료 · 나머지는 열린 안전칸 수로 순위 확정
    R->>D: 결과·공동 순위·이탈 여부 저장
    R-->>H: 순위 전달
    R-->>P: 순위 전달
```

```mermaid
stateDiagram-v2
    state "방 대기" as Waiting
    state "보드와 좌석 준비" as Preparing
    state "최초 연결·카운트다운" as Countdown
    state "진행 중" as Playing
    state "결과" as Results
    state "방 종료" as Closed
    [*] --> Waiting
    Waiting --> Preparing: 전원 준비와 방장 시작
    Preparing --> Countdown: 준비와 입장 기록 성공
    Preparing --> Waiting: 준비 실패 또는 취소
    Countdown --> Waiting: 최초 연결 기한 초과 또는 권한 철회
    Countdown --> Playing: 전원 연결과 서버 시작 시각 도달
    Playing --> Results: 완주·시간 만료·이탈 판정
    Results --> Waiting: 같은 방에서 다음 판 준비
    Waiting --> Closed: 빈 방 또는 만료
    Closed --> [*]
```

공격 대상은 서버가 무작위로 고르고 성공한 지목은 원래 공격자에게 반사한다. 재대결은 같은 방에 새 라운드와 새 보드를 만든다. 서버 재시작 시 메모리의 방은 닫히며, 영속 입장 기록은 장애 판정에 사용한다. [다인전 상세](20-multiplayer-rooms.md) · [4명 공격·재대결 검증](../verification/249-multiplayer-integration.md)

### 권위 입력과 재접속

```mermaid
sequenceDiagram
    actor B as 참가 브라우저
    participant S as 서버 매치
    participant C as Rust 코어
    B->>S: 명령 ID·순번·연결 epoch와 함께 입력
    S->>S: 세션·좌석·명령 순서·중복 확인
    S->>C: 현재 라운드에 유효한 조작 적용
    C-->>S: 권위 상태와 입력 결과
    S-->>B: 입력 확인과 해당 참가자에게 공개된 상태
    Note over B,S: 연결이 끊겨도 서버의 경기 시간은 흐름
    alt 서버 유예 안에 재접속
        B->>S: 유효 세션으로 다시 연결
        S-->>B: 새 epoch·스냅샷·본인 입력 순번
        B->>S: 확인을 받지 못한 명령 재전송
        S-->>B: 이미 처리한 명령이면 원래 응답
    else 유예 만료
        S->>S: 참가 모드와 남은 참가자에 맞는 이탈 판정
    end
```

[WebSocket 계약](06-protocol.md) · [자동 재접속](../verification/80-bounded-online-reconnect.md) · [연결 종료·재시작 검증](../verification/107-reconnect-integration-exit.md)

## 데일리와 오프라인

```mermaid
sequenceDiagram
    actor B as 브라우저
    participant W as WASM · IndexedDB
    participant R as Rust 데일리 API
    participant D as PostgreSQL
    alt 공식 시간 도전
        B->>R: POST /api/v1/daily/start
        R-->>B: 승인된 UTC 판 정의와 서버 시간 티켓
        B->>W: 승인된 규칙으로 풀이하고 replay 기록
        loop 도전 중
            B->>R: heartbeat 전송
        end
        B->>R: submit-timed에 티켓과 replay 제출
    else 로컬 데일리 후 연결 복구
        B->>W: 캐시된 규칙으로 연습하고 개인 후보 저장
        B->>R: 사용자가 명시 제출 · 티켓 없이 replay 전달
    end
    R->>R: 규칙 버전과 replay 검증 · 서버 시간 증거 확인
    alt 첫 유효 완료
        R->>D: 계정과 데일리별 첫 완료 저장
        D-->>R: 공식 완료 확인
    else 이미 공식 기록 있음
        R->>D: 기존 공식 완료 조회
    end
    R-->>B: 공식 기록과 검증 수준
```

서버 시간 증거가 유효할 때 시간 순위에 들어가며, 완료만 검증한 기록은 완료·오류 순위로 확인할 수 있다. 개인 기록과 제출 대기는 별도로 유지한다. [공식 데일리 화면과 서버 검증](../verification/132-official-daily-flow.md) · [공개 순위](../adr/0058-public-daily-rankings.md)

```mermaid
flowchart TD
    Open["웹 또는 PWA 실행"] --> Online{"연결 가능?"}
    Online -->|"가능"| Web["웹 로드 · Service Worker가 공개 자산 캐시"]
    Online -->|"끊김"| Cache{"공개 앱과 WASM이 캐시됨?"}
    Cache -->|"있음"| Play["로컬 연습과 데일리"]
    Cache -->|"없음"| First["연결 후 다시 접속"]
    Play --> IDB["개인 결과 · 제출 대기 후보를 IndexedDB에 보관"]
    IDB --> Reconnect["연결 복구 후 로그인 · 명시 제출"]
    Reconnect --> Verify["서버 검증 후 공식 기록 가능"]
```

세션·개인 API·광고 응답은 Service Worker에 캐시하지 않는다. [브라우저 저장소와 오프라인 경계](08-client-architecture.md#오프라인-경계) · [오프라인 검증](../verification/14-public-offline-cache.md)

## 광고와 자체 수집 동의

```mermaid
flowchart TD
    Screen{"현재 문서"}
    Screen -->|"허용 공개 홈·규칙·동의 관리"| Tag["Google 설정이 있으면 태그 준비 · 광고 요청 일시중지"]
    Screen -->|"게임·로그인·개인 등 보호 문서"| Protected["Google 태그 없는 문서"]
    Tag --> Choice{"홈·규칙에서 현재 광고 동의 확인?"}
    Choice -->|"예"| Ads["인페이지 자동 광고 요청 허용"]
    Choice -->|"거부·미확인·오류 또는 동의 관리"| Pause["광고 요청 일시중지 유지"]
    Ads -->|"철회·만료"| Pause
    Tag -->|"보호 화면으로 이동"| Protected
    Session["현재 로그인 세션에서 별도 자체 수집 선택"] --> Consent["서버가 현재 목적·동의 revision 검증"]
    Consent -->|"저장 시점에도 유효"| Event["선택 이벤트 저장"]
```

Google 광고 선택과 자체 수집 선택은 별개다. 보호 화면으로 갈 때 Google 태그를 시작한 문서에서 새 문서로 이동한다. 광고가 비활성이거나 실패해도 게임이 진행된다. [광고·동의 설계](11-ads-consent.md) · [공개 문서와 보호 문서 검증](../verification/235-public-launch-wiring.md)

## PostgreSQL ERD

현재 [마이그레이션](../../migrations)의 테이블 이름과 물리 FK를 사용한다. 관계와 주요 제약을 읽는 데 필요한 컬럼을 표시한다. 실선은 자식의 기본 키에 부모 키가 포함되는 관계이며, FK가 nullable인 경우 부모 쪽의 선택성을 표시한다.

### 계정·인증·동의

```mermaid
erDiagram
    auth_accounts ||..o{ auth_identities : "인증 수단"
    auth_accounts ||..o{ auth_sessions : "로그인 세션"
    auth_accounts o|..o{ auth_transactions : "연결 또는 재인증"
    auth_identities ||--o| auth_credentials : "Apple 철회용"
    auth_sessions ||--o| session_consents : "현재 목적별 동의"
    auth_sessions ||..o{ optional_product_events : "선택 이벤트"

    auth_accounts {
        uuid id PK
        text nickname
    }
    auth_identities {
        uuid id PK
        uuid account_id FK
        text provider
        bytea subject_digest
        integer digest_key_version
    }
    auth_sessions {
        uuid id PK
        uuid account_id FK
        bytea token_hash UK
        timestamptz expires_at
    }
    auth_transactions {
        uuid id PK
        uuid account_id FK "로그인에서는 null"
        bytea state_hash UK
        timestamptz expires_at
    }
    auth_credentials {
        uuid identity_id PK,FK
        text provider FK "identity_id와 복합 FK"
        bytea encrypted_refresh_token
    }
    session_consents {
        uuid session_id PK,FK
        uuid revision
        boolean analytics
        boolean advertising
    }
    optional_product_events {
        uuid id PK
        uuid owner_session_id FK
        uuid consent_revision
        text purpose
    }
```

선택 이벤트는 **세션**에 연결된다. `consent_revision`은 당시 저장에 사용한 값이며 현재 동의 행과의 FK는 없다. [인증 DDL](../../migrations/202610080001_auth_foundation.sql) · [동의 DDL](../../migrations/202611010001_session_consents.sql) · [선택 이벤트 DDL](../../migrations/202611030001_optional_product_events.sql)

### 온라인 대전·데일리·필수 이벤트

```mermaid
erDiagram
    online_active_matches ||--o{ online_active_players : "입장 기록의 좌석"
    online_match_results ||--o{ online_match_players : "결과의 좌석"
    auth_accounts o|..o{ online_active_players : "사람 좌석"
    auth_accounts o|..o{ online_match_players : "사람 좌석"
    auth_accounts ||..o{ daily_completions : "공식 첫 완료"
    daily_definitions ||..o{ daily_completions : "공개 UTC 판"
    auth_accounts ||..o{ product_events : "필수 이벤트"
    online_match_results o|..o{ product_events : "대전 원본"
    daily_completions o|..o{ product_events : "데일리 원본"

    online_active_matches {
        uuid id PK
        uuid room_id "nullable"
        smallint participant_count
        timestamptz admitted_at
        timestamptz started_at "nullable"
    }
    online_active_players {
        uuid match_id PK,FK
        smallint seat PK
        smallint participant_count FK
        uuid account_id FK "봇은 null"
    }
    online_match_results {
        uuid id PK
        uuid room_id "nullable"
        smallint participant_count
        text rules_hash
        bytea secret_seed "서버 전용 · 만료 후 null"
        timestamptz retention_started_at
    }
    online_match_players {
        uuid match_id PK,FK
        smallint seat PK
        smallint participant_count FK
        uuid account_id FK "봇은 null"
        smallint rank "nullable"
        boolean tied
        boolean forfeited
    }
    daily_definitions {
        text id PK
        date utc_date
        integer seed_version
        jsonb rules_snapshot
    }
    daily_completions {
        uuid id PK
        uuid account_id FK
        text daily_id FK
        uuid attempt_id
        bigint verified_elapsed_ms "nullable"
    }
    product_events {
        uuid id PK
        uuid account_id FK
        uuid online_match_id FK "nullable"
        uuid daily_completion_id FK "nullable"
    }
```

- 두 참가자 테이블의 PK는 각각 `(match_id, seat)`이고 `(match_id, participant_count)`가 부모의 `(id, participant_count)`와 맞아야 한다. `room_id`는 서버 메모리의 방 식별자다. 계정 삭제는 사람의 참가 행을 제거한다.
- 데일리는 `(account_id, daily_id)`로 첫 기록을 한 건 유지한다. 필수 이벤트는 대전 결과와 데일리 완료 중 하나만 원본으로 참조한다.
- 원시 필수 이벤트를 집계한 `product_metric_aggregates`는 `(metric, period)`로 저장하며 사용자나 이벤트 FK가 없다. 복구 제어·원본 식별과 삭제 원장은 [복구 스키마와 절차](../workflow/recovery-quarantine.md)를 따른다.

[다인전 결과 DDL](../../migrations/202611220001_multiplayer_race_results.sql) · [데일리 최초 완료 DDL](../../migrations/202610110001_daily_first_completion.sql) · [필수 이벤트 DDL](../../migrations/202610150001_essential_product_events.sql)

### 데이터 보존과 삭제

```mermaid
flowchart TD
    Match["대전 결과 저장"] --> Seed["원래 7일 경계에 서버 전용 시드 제거"]
    Match --> History["원래 90일 경계에 전적 원본 정리"]
    Daily["공식 데일리 첫 완료"] --> Replay["완료 7일 후 replay 원문 제거"]
    Daily --> History
    Raw["필수·선택 이벤트 원시 행"] --> Prune["원래 발생 시각 30일에 원시 행 정리"]
    Raw -->|"필수 이벤트 기반 운영 집계"| Stats["익명 집계는 13달력개월 보존"]
    Delete["계정 삭제"] --> Cascade["소유 세션·동의·참가·완료와 연결 이벤트 정리"]
    Delete --> Ledger["삭제 원장으로 백업 복구 시 삭제 반영"]
```

복구가 원래 보존 기한을 늘리지 않도록 원본 시작·완료 시각을 사용한다. 계정 삭제 뒤에는 식별할 수 없는 대전 부모 행과 익명 집계가 각 보존 정책에 따라 남을 수 있다. [원본 정리 검증](../verification/215-source-retention.md) · [익명 지표 보존 검증](../verification/145-anonymous-metrics-retention-and-operations.md)

## 기동, 백업, 부하 확인

```mermaid
flowchart TD
    PG["PostgreSQL 기동과 health 확인"] --> Migrate["소유자 역할로 migration"]
    Migrate --> Grant["운영 역할에 필요한 권한 적용"]
    Grant --> Server["Rust 서비스 기동 · 복구 제어와 DB 확인"]
    Server -->|"ready"| Web["Nginx 기동"]
    Web --> Tunnel["cloudflared 연결"]
    Tunnel --> Smoke["도메인에서 웹·API·WS 확인"]
```

```mermaid
flowchart TD
    Active["운영 DB"] --> Dump["암호화 백업 · 별도 매체"]
    Dump --> Restore["격리된 PostgreSQL에 복원"]
    Restore --> Verify{"최신 권리 원장·원본 관계 확인?"}
    Verify -->|"확인됨"| Seal["원래 writer 차단"]
    Verify -->|"불충분"| Hold["대상 격리 유지"]
    Seal --> Retention["대상에 최신 삭제·해제와 보존 정리 적용"]
    Retention -->|"검사 통과"| Resume["대상만 명시 재개 · ready 확인"]
    Retention -->|"실패"| Hold
```

```mermaid
flowchart TD
    Setup["검증 호스트에서 배포 설정으로 실행"] --> Load["1대1·4인전을 각각 낮은 동시 경기 수부터 반복"]
    Load --> Measure["응답 p95·오류율·CPU·메모리·대기열과 VM 송신량 기록"]
    Measure --> Pass{"성능 기준 통과?"}
    Pass -->|"통과 · 다음 단계 있음"| More["동시 경기 증가"]
    More --> Load
    Pass -->|"한계 또는 시험 상한 도달"| Stable{"반복해서 통과한 단계 있음?"}
    Stable -->|"있음"| Admit["검증된 안정 부하의 70% 이하로 초기 입장 상한 산정"]
    Stable -->|"없음"| Inspect["병목 확인 후 재검증"]
    Measure --> Traffic["원본 송신량을 방문·대전 패턴으로 환산해 월 전송량 추정"]
```

Compose 기본값은 경기 16슬롯·WebSocket 32개다. 이 값은 호스트의 검증된 성능 수치와 별도로 표시한다. 1대1 부하 도구와 4인 통합 검증은 각각 존재하며, Oracle 또는 실제 운영 장비의 4인 부하 검증은 별도다. [기동·종료 절차](../workflow/container-deployment.md) · [격리 복구](16-observability-ops.md) · [성능 기준](13-performance.md) · [부하 도구](../workflow/release-operations.md)

## 요구와 검증 연결

| 그림 | 대표 요구와 시나리오 | 검증 근거 |
|---|---|---|
| 구조·사용자 여정 | FR08/10/11 · NFR01/02/06 · TS13/18/19/22/26/30/36 | [로컬 연습](../verification/12-local-practice-tutorial.md) · [PWA](../verification/221-installable-pwa.md) |
| OAuth·계정·동의 | FR01/12/13/16 · NFR02 · TS01/20/23~25/29/31~36 | [계정 권리](../verification/45-account-rights-apple-revocation.md) · [내 정보](../verification/178-my-info-pages.md) · [Google 연결](../verification/235-public-launch-wiring.md) |
| 온라인 대전·재접속 | FR02~07/14/15/17/18 · NFR02 · TS02~12/14~16/21/22/27/37~45 | [거짓말 인증](../verification/06-lie-certification.md) · [1:1](../verification/70-online-quick-match-ui.md) · [4인](../verification/249-multiplayer-integration.md) |
| 데일리·오프라인 | FR09/10 · TS17/18/36 | [공식 데일리](../verification/132-official-daily-flow.md) · [오프라인](../verification/14-public-offline-cache.md) |
| PostgreSQL·운영 | FR16/18 · NFR03/04/05/07/08 · TS21/27~30/35/43/45 | [보존](../verification/215-source-retention.md) · [격리 배포](../verification/211-isolated-container-deployment.md) · [운영 도구](../verification/231-release-operations.md) |

[전체 요구 추적표](00-review-checklist.md) · [시나리오](../product/19-test-scenarios.md) · [Git 검토·병합 규칙](../workflow/git-workflow.md)
