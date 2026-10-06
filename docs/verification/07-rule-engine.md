# #7 단일 규칙 snapshot·매치 전이 검증

FR03/04/05/15 · TS04/05/06/08/09/14/22/27. [ADR0011](../adr/0011-rule-snapshot-and-transitions.md), 코드 `1bb88c89d1768fe1bca2a5e0a60e76bcaae27fbf` 기준. #6은 PR33으로 병합·종료한 뒤 시작했다.

## 관찰 가능한 행동

Red에서 TOML loader가 항상 거절해 유효 설정/공백 hash 검사에 실패했고, RuleEngine의 행동 stub이 open/mine 시나리오에 실패했다. 종료 뒤 clock이 계속 감소하는 실패(239997→233000ms)와 zero opening만 있는 막힌 보드를 수락하는 실패를 실행 확인한 뒤 수정했다.

추가 리뷰에서 숨은 공격 직후 defender의 공통 revision이2→3으로 늘어나는 시점 누출을 Red로 확인했다. 플레이어별 공개 revision으로 바꿔 같은 tick의 공격/상대 flag 전후 전체 defender projection이 동일하게 유지된다. 전송의 server_seq도 공개 stream별로 분리하는 계약을 추가했다.

Green에서 새 규칙3개·매치16개를 포함한 core53개 테스트가 통과했다.

- 설정 원천은 [game-rules.toml](../../config/game-rules.toml). 타입/unknown key·모든 운영 범위·version/strategy·semantic hash/변조를 검사한다. 기본 BoardSpec도 이 snapshot에서 받는다. Rust GameRules/RulesSnapshot→ts-rs 생성 TS의 freshness/typecheck가 통과했다.
- 동일 보드·zero opening·초기 gauge0, 재열기/중복 command의 gain0, flag 셀 거절과 flood 건너뛰기, flood의 새 안전칸별 gain과 cap.
- 공개 후보 없음/두 closed overlay capacity에서 gauge 보존, 성공시 gauge0, 공격 직후 defender projection 불변, opened false number→정확한 지목으로 복원·source gauge0/2초 stun. 반복 command는 반사 중복 없음, 새 command로 재지목하면 본인3초 stun.
- mine3초 stun/gain0, 기절 종료와 같은 tick의 입력 허용, 긴 기존 stun을 짧은 반사가 줄이지 않음.
- countdown 입력 거절, 동일 tick batch의 seat/seq 순서와 clear 최초 commit 승리, deadline 동일 tick 입력 거절, progress timeout 승패/동률 draw, 종료 clock 고정.
- sequence/ID conflict·epoch·8192기본 명령 한계·시각 역행 거절, rejection도 dedup, 새 epoch 재접속 후 원래 ack 재전송. 기본 한계는4분+30초20/s의5400보다 크다.
- reconnect grace 동일 tick 거절, 한 명 만료 forfeit·둘 모두 만료 abandon, 시간 건너뛰기에서도 실제 가장 이른 terminal tick 보존, countdown disconnect 취소·server abort·종료 불변.
- Board/snapshot/spec/overflow와 일반 노게스 인증 실패를 시작 전에 거절. 자동 opening이 이미 둘 모두를 완료했다면 시작 tick clear draw.
- 분석은 별도 순수 job이며 공개 숫자 revision/관측/전체 이력이 일치할 때만 commit한다. 오래된 job 거절, 주 입력 전이에는 DFS가 없다. 실제 worker와 네트워크 serial actor는 #16 검증 대상이다.

## 검사와 제한

전체 scripts/check.ps1에서 fmt/clippy·workspace tests·WASM target check·TS 생성/freshness·lint/typecheck·웹 unit/build·desktop/mobile Chromium2개·문서/Mermaid34개 모두 통과했다. PostgreSQL은 필수 CI에서 실제 실행하며 로컬 기본 테스트의 ignored1개를 통과로 집계하지 않는다. 공개 타입/도메인 타입과 secret Board/overlay/analysis를 분리했고 정답·상대 셀을 public projection에 추가하지 않았다.

cargo-llvm-cov0.9.1 core source 전체(테스트/예제 파일 제외): line1952/1997=97.7466%, function165/169=97.6331%, region2997/3114=96.2428%. Rules line100%, Game99.4872%. [측정 요약](rules-coverage-summary.json). branch/전체 앱 coverage·실호스트 지연·온라인 WS/DB 매치 동작은 이번 core 측정이 아니다.

의존성은 버전 인자 없이 cargo add한 최신 serde1.0.229/serde_json1.0.151/TOML1.1.6/BLAKE3 1.8.7/ts-rs12.0.1과 lockfile로 재현한다. 최신 버전 사용과 lockfile은 서로 다른 역할이다. 공개 수치에 정답 seed/Board hash를 넣지 않으며 snapshot hash는 공개 규칙만 식별한다.

에이전트 리뷰에서 입력 시간 경계·dedup/capacity·전이 원자성·source 반사·종료 우선순위·분석 이력 binding·설정 단일 원천을 확인했다. 최신 PR head의 필수6개 checks를 확인하고 사용자의 위임에 따라 병합한다.
