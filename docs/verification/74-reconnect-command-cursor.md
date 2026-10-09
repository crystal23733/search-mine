# #74 재접속 snapshot의 본인 입력 순번 복구

대응: FR14/NFR01/02 · TS15/16/20/21 · [ADR0035](../adr/0035-reconnect-command-cursor.md). 선행 #17/#72·PR73/develop1b619ec2 병합·종료 후 시작했다.

## 구현과 경계

Rust RuleEngine.last_sequence는 seat별 실제 소비 순번을 읽기만 한다. MatchState는u32::MAX 초과를 exhausted로 투영하며 actor가 snapshot.last_client_seq에 본인 값만 넣는다. Rust→TS 생성 계약과 닫힌 웹 decoder를 함께 갱신했다. 새 controller는 snapshot cursor 다음부터 입력하며 u32 최대에서는 새 command를 전송하지 않는다. 새 epoch는 command cache와 소비 순번을 초기화하지 않는다. 게임 규칙·seed·공개 판/상대 정보·기절/시간 규칙·의존성/스키마는 변경하지 않았다.

## 실제 실행

웹에서는 cursor41을 받아도client_seq1을 보내는1failed/12passed Red(1.69s)를 확인했다. 실제 TCP WS는 snapshot의 cursor가Null인1failed Red(0.01s)였다. 수정 후 관련 웹24개(1.90s), native state13개와 실제 TCP WS11개(0.07s)가 통과했다. 실제 PostgreSQL17.4/Rust/HTTPS PC1440/mobile390의 reload2개(16.4s)가 통과했다. flag 입력→7초 경과→reload→같은 match/새 epoch/소비 cursor·남은 시간/flag 복원→순번2 새 입력 Applied→실제 Result saved를 확인했다. 상대 보드/flag·snapshot 수명은 유지되고 온라인 match/secret을 일반 저장소에 기록하지 않았다.

WS의 이전 epoch는 MatchConnection 전송 경계에서 공개 error 후 연결을 종료한다. 최초 시험이 이를 도메인 ACK로 기다려 ConnectionAborted가 났다. 실제 포트 계약을 읽어 마지막 단계에서 error를 검증하고 그 전에 정상 새 순번 입력/상태를 검증하도록 시험을 보정했다. 서버의 epoch 처리와 timeout/retry를 바꾸지 않았다.

최종 scripts/check.ps1(10324)은 exit0이었다. locked fmt/all-target Clippy/native/WASM·생성 TS/fixture/tokens·웹 format/lint/typecheck/전체 unit/build·실제 HTTPS 포함 browser77개(1.9m)·문서127개/Mermaid34개 오류0을 확인했다. 웹 lines1955/2243=87.16%다. 로컬 native 기본 실행에서 ignored 처리된 DB 검사는 실제 DB 실행 성공으로 계산하지 않는다. CI의 실제 PostgreSQL18·최신 head/병합 상태는 PR에서 별도로 확인한다.

## 리뷰와 제한

첫 CI37926080244에서 웹 unit141개·실제 PG18.6 DB26개(ignored0/4.75s; coverage 재실행7.47s)가 통과했다. Auth99.48/Match99.29/Lobby98.37/whole server92.68%다. browser는77개 중74passed/기존offline-mobile 두 탭 update1failed/의존measurement2미실행(4.2m)이었다. 최초·재시도 모두 idle update 뒤 두 탭 reload를 기다리다 실패했으며 trace를 보존했다. 같은 PC/mobile 시나리오의 로컬 반복8개는 통과(35.5s)해 원인을 재현했다고 주장하지 않는다. 테스트에 실제 클릭·prepare/release·reply의 수동적 관측/첨부를 추가하고 reload 관측만10초로 제한해 실패 때 진단 수집 시간을 남긴다. 제품 SW/캐시/권한/조정 timeout·retry는 바꾸지 않는다. 새 CI에서 확인 전 병합하지 않는다.

pm-ai-shipping code-review correctness를 scope74/base1b619ec2에 순차 적용했다. 실제 소비 순번과 요청/ACK 개수를 다르게 만든 점프9→11·cached 거절/중복/잘못된 epoch/sequence/future revision, 동일 UUID의 새 epoch·변조 retry/원본 ACK revision과 seat별 cursor, 재로드에서 actual snapshot→새 입력의 계약을 강제 확인했다. 내부u64→u32 범위도 wrap 없이 입력 중단을 확인했다. actor가 같은 직렬 attach 처리에서 epoch/cursor/view를 만들고, 새 계정/매치/연결 세대의 late reply 차단을 유지함을 읽었다. 현재 생산자는 attach당 snapshot을 한 번 보낸다. 미래 자동 재접속의 예약 순번/미확인 큐는 후속 검증 범위다. 검토 범위에서 추가로 뒷받침되는 결함은 없었다. 별도 성능·보안 감사를 수행했다고 보고하지 않는다.

자동 backoff·메모리 미확인 command 재전송·서버 grace 공개 상태/30초 화면, 영속 active match journal·재시작 abort/결과 재조회는 후속 #18 하위 작업이다. 이 변경만으로 상위18/서버 장애 복구를 완료했다고 보고하지 않는다. 운영16×16/미니PC26·실제 제공자42·사람27은 미측정이다.
