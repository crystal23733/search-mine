# #8 공개 관측 봇 검증

FR06 · TS11. [ADR0012](../adr/0012-public-observation-bot-policy.md), 코드 `b4c663646f648c7a52aed2a296e5a4feacb7bfef`. 선행 #7은 PR34 병합/이슈 종료 확인 후 시작했다.

BotPolicy의 입력은 공개 Projection·Clock·별도 RandomSource이며 숨은 Board/seed/상대 cells/overlay를 받지 않는다. 공개 이력 해석은 canonical 예산, 다음 safe 계획은 난이도 예산으로 분리했다. 화면의 거짓 숫자를 작은 예산 탓에 truth로 취급하지 않는다. 해당 난이도에서 완료한 safe 증거는 보존하고 열린 셀만 제거한다. 내부 추론 결론에는 Serialize/Debug를 추가하지 않았다.

Red에서 선택 stub의 None≠Open(8), 잘못된0노드 프로필 수락을 실행 확인했다. Green은 봇10개를 포함한 core63개 테스트다.

- 다른 숨은 보드 `[5,7]`/`[2,6]`의 동일 공개 관측·Clock/RNG에서 동일 의도.
- config의 쉬움900/보통500/어려움180ms 결정 간격, 정확한 경계 tick에 행동 재개.
- 쉬움 계획 예산을1제약으로 낮춰도 공개 이력의 실제 lie를 정확히 식별하며1500ms 이전 지목0, 정확한 deadline에서 지목한다.
- safe에 잘못 놓인 flag는 unflag→다음 결정에서 open. flag를 mine 증거로 사용하지 않는다.
- 게이지가 가득 찼어도 같은 공개 상태에서 공격 실패를 매 tick 반복하지 않고 safe를 연다. 상대 공개 progress 변경 뒤 재시도한다.
- 카운트다운/기절/종료/시각 역행·잘못된 flag 길이/hash/이력/관측/prefix 거절. invalid snapshot으로 기존 knowledge를 교체하지 않는다.
- 계획 예산 초과에서 대기. 막힌 fixture에 최초 확정 safe가 있으므로 그것을 먼저 연 뒤, 더 이상 확정 safe가 없는 관측에서 추측0을 검사했다.
- seed0~15의 실제16×16/40 인증 보드에서 어려움 봇이 공개 Projection만으로 행동하고 모두 clear 승리했다. fake Clock200ms tick·별도 bot RNG seed4000~4015이며 실제 사람/서버의 승률·반응 시간 표본이 아니다.

세 프로필 수치는 [game-rules.toml](../../config/game-rules.toml)에서 읽으며 범위·난이도 순서를 검증한다. Rust BotSettings/BotProfiles→TS를 생성했다. 난이도별 재미·인간 이해와 승률은 #27에서 확인하고 실제 CPU worker/온라인 actor는 #12/#16에서 연결한다.

scripts/check.ps1의 fmt/clippy·전체 Rust·WASM target·TS freshness·lint/typecheck·web unit/build·desktop/mobile E2E2개·문서/Mermaid34개 모두 통과했다. PostgreSQL ignored 테스트는 로컬 통과로 계산하지 않고 실제 필수 CI에서 검사한다.

cargo-llvm-cov0.9.1 core source 전체(테스트/예제 제외): line2126/2173=97.8371%, function181/185=97.8378%, region3260/3382=96.3927%, Bot line98.5294%. [측정 요약](bot-coverage-summary.json). branch/전체 앱 coverage·미니 PC 성능은 측정하지 않았다.

에이전트 리뷰에서 숨은 데이터 없는 입력 타입, 이력 분류와 난이도 예산의 경계, 불완전 모델 거절, 불변 safe 증거 보존, timing/rate·실패 공격 후 진행을 확인했다. 최신 PR head의 필수6개 checks 통과 후 사용자 위임에 따라 병합한다.
