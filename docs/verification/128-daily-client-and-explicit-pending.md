# #128 데일리 client와 명시적 대기 제출

[이슈128](https://github.com/crystal23733/search-mine/issues/128) · [ADR0060](../adr/0060-daily-client-and-explicit-pending.md) · M3/상위19.

선행126/PR127 developa5dbdec 병합·126 CLOSED 뒤 설계부터 작성했다. main placeholder를 실제 API로 교체하고 현재 계정 확인 Dialog/8언어 설정 버튼을 추가했다. 로그인/연결 회복만으로 자동 flush하지 않는다. Rust 생성 DTO/RulesSnapshot와 기존 전후 proof를 재사용하고 opaque server sessionRevision은 메모리로만 전달한다. 클라이언트 규칙/hash/seed/공식 시간 판정과 인증 저장소 필드는 추가하지 않는다.

## 실제 검증

최종 scripts/check.ps1 exit0: Rust399·웹237개/40파일/줄89.38%(2687/3006)·Daily client 140/143=97.90%·공통 wire/rules/JSON reader100%·브라우저109개/4.0m/재시도0·문서178/Mermaid34 실패0.

실제 HTTPS production 설정의 제출 버튼 부재 Red(exit1)를 확인했다. 초기 production /src import 준비 오류는 별도 보존하고 Red로 세지 않았다. production HTTPS PC/mobile 각8언어 명시 확인→서버 첫 완료/Practice·503 재시도→pending0/개인record1을 확인했다. 별도 typed 모든메서드 실제HTTPS/PG 시험은 APIRequest cookie jar adapter를 사용하며 stream/abort/UTF-8는 단위 fake stream으로 검사했다. 단위에서 cookie A→B를 accepted/expired/unsupported 각각에 주입하여 모든 terminal post-proof 실패 시 후보 보존, generic400/409/손상410/503 보존을 확인했다. 입력8192/2MiB·응답64KiB·고정URL·opaque 메모리 revision/전후 proof·public omit/2slot/10초/ordinal/secret제외를 검증했다.

최종 초점 Green: typed 모든API와 production PC/mobile8언어·503보존/재시도3개/17.9초, UI port4개/2.10초. 최초 전체는91통과/1실패/17미실행/3.7분이었다. 추가한503 상태 검사가 preferences 저장 상태와 제출 보존 상태를 함께 찾는 strict locator 오류였으며 실제 보존 메시지/후보가 남아 있었다. 실패 로그/trace를 보존하고 제출 상태 문구로 한정한 뒤 같은 timeout·retries0으로 전체를 재실행했다. 초기 단위의 잘못된 문구/App bootstrap 관측과 HTTPS adapter의 /me/identities 누락은 별도 로그에 보존 후 수정했다. timeout/retry 완화나 구버전 의존성은 사용하지 않았다. local full은 새 liar_daily128fullb DB/기존 migration으로 실행했고 기존 DB/원문/main을 유지했다. 추가 의존성·DB migration·Rust 제품 코드/프로토콜 wire 변경은 없다.

## 코드 리뷰·후속

pm-ai-shipping:code-review correctness/changes/a5dbdec 순차 리뷰: Practice 원래 attempt/Some time과 이번 요청 차이·계정/epoch/metadata/trust·종료 결과 post-proof·취소/크기/수명·기존 online 계약·명시UI/activity·pending만 제거와 개인기록/secret storage 경계를 확인했다. 추가 근거 결함 없음. 독립 보안 감사/실계정/실기기/인간/RTT 검증을 수행한 것은 아니다.

원격CI/정확head squash merge·#128 CLOSED는 최종 확인 단계다. 공식 시작/controller/heartbeat·두 순위 화면과 상위19 전체는 후속이다. OAuth42·운영25/26·사람27·원인 미확정83은 미완료다.
