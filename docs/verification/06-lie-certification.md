# #6 두 거짓말 증명과 공개 후보 전략 검증

FR04 · TS06/07. 최초 모델의 실패와 채택한 전략의 결과를 분리한다. 이 결과는 모든 임의 lie 배치를 허용하는 게임의 완주·재미·실호스트 성능을 보장하지 않는다.

## 실행과 증거

초기 Red에서 일반 truth-only 솔버를 lie 관측에 적용하면 안전하지 않은 칸을 safe로 반환했고, 공개 mine/global count로 확정 가능한 lie를 모순으로 처리했다. certificate를 항상 거절하는 stub도 두 overlay 시나리오에서 실패했다. 이력 replay의 빈/no-op 배치를 허용하는 실패를 실행 확인한 뒤 길이512와 유효한 변경 배치 검사를 추가했다.

Green 기준 core34개 테스트: 3×3/2mine의18,432개 truth 관측, 3×2/2mine의6,784개 lie 관측·2,928개 후보와 미래 세계 검사, 두 lie 공개/닫힘 순서, 같은 관측의 서로 다른 숨은 세계, 전역 mine/lie 수의 구성요소 결합, proptest, budget·모순·stale certificate·projection rollback·이력 replay가 통과했다. 공개 후보 전략도 작은 판의 합법적인 이력에서 모든 독립 숨은 세계의 target/이웃 상태를 대조하고16×16의32개 인증 seed에서 두 overlay 적용과 전체 완주/replay를 검사한다. 검사한 표본의 반례는0이다.

KnowledgeSolver와 PolicyKnowledge는 Observation/본인 공개 이력만 받는다. Board는 모델 증명 뒤 독립 대조와 실제 open 실행에만 사용한다. 초기 zero opening의 truth는 공격 전에 공개하는 순서로 증명하며 임의 snapshot 숫자를 초기 truth로 받아들이지 않는다. flags는 증거에 사용하지 않는다. Board·certificate·논리 관측·후보 등록 목록에는 Serialize/Debug를 추가하지 않았다.

## 최초 임의2lie 모델의 실패

코드 `1c72059c7c9156cff59fa6e895081f6bf2835e48`, seed0~999, 일반 노게스 인증16×16/40 판. 초기 공개 opening만 trusted로 기록하고 다른 열린 비제로 숫자는 ±1/최대2lie일 수 있게 했다.

| 항목 | 실제 결과 |
|---|---:|
| 완주 / 막힘 / budget | 0 / 999 / 1 |
| 검사한 상태 | 3,446 |
| 닫힌 비제로·유효 delta 후보 옵션 | 611,907 |
| 확정 인증 옵션 | 781 (0.1276%) |
| 추론 준비 p50 / p95 / max | 59 / 432 / 17,180µs |

[원시 CSV](generic-lie-1000.csv), SHA256 `9c53cbfe744b7c7c1b7273aca26dffc154f8ef01032614d35e4e7fdec68d7f35`. 안전성 반례0만으로 정상 판의 진행 가능성을 주장할 수 없어 이 모델을 제품 경로에 그대로 적용하지 않았다.

## 공개된 known-neighborhood-v1 전략

[ADR0010](../adr/0010-public-certified-attack-strategy.md)의 공개 후보 제한을 모델 조건으로 포함한다. 공개 이력으로 이미 safe와 모든 이웃 상태가 확정된 닫힌 비제로 셀만 등록한다. 등록되지 않았던 새 숫자는 공격될 수 없어 truth이며 등록된 셀의 truth는 이전 공개 증거로 고정된다. 이 귀납 불변식으로 거짓 화면 값을 유지하면서 안전 풀이 경로와 open→확정 지목 증명을 제공한다. 후보가 없거나 budget/모호성/capacity/stale이면 공격을 거절한다. 게이지 보존·실제 공격/기절 규칙은 #7에서 검사한다.

코드 `86c63c7c60c9c5c33d3b0c01a99f109804cbc033`, seed0~9999, RNG1/solver1, 기본200,000노드·22칸·1024제약. Rust1.99.0, release thin LTO/codegen-units1, AMD Ryzen9 3900X/32GiB/Windows11 개발 PC. 중간에 coverage·검사도 실행했으므로 격리 성능 측정은 아니다.

| 항목 | 실제 결과 |
|---|---:|
| 완주 / 실패 | 10,000 / 0 |
| 풀이 상태 | 133,900 |
| 닫힌 비제로·유효 delta 후보 옵션 | 13,409,793 |
| 인증 옵션 | 208,518 (1.55497%) |
| 실제 적용 / 정확한 지목 | 85,959 / 85,959 |
| 두 overlay가 동시에 존재한 상태 | 32,377 |
| 공격0인 seed | 0 |
| observe+refresh p50 / p95 / max | 36 / 61 / 568,395µs |

[원시 CSV](policy-lie-10000.csv), SHA256 `972075af0aa5f3a0001fcd07e579a32c1c454c02b5277399a3596e66ccdd591e`. 매 상태에 반복 등장하는 셀/delta도 분모에 포함한다. 이 비율은 후보 옵션의 인증률이며20게이지·인간 입력·기절을 포함한 실제 대전 공격 성공률이 아니다. harness는 가능한 overlay를 최대2개 적용하고 알려진 lie를 수정한 뒤 반복하며 모든 seed의 공개 이력을 재생해 원래 knowledge와 같은지 확인한다. 인간 플레이의 재미와 공격 빈도는 #27 검증 대상이다.

```powershell
cargo run --locked --release -p liar-core --example lie_bench -- 1000 .tmp/generic.csv
cargo run --locked --release -p liar-core --example policy_bench -- 10000 .tmp/policy.csv
```

최대568ms 분석 지연이 있어 온라인 입력에서 동기 분석하지 않는다. 이전 immutable proof를 유지하고 bounded worker 결과를 해당 revision에만 적용한다. 미니 PC·전체 match 입력 지연·WASM 재현은 후속 이슈에서 검증한다.

## 커버리지·전체 검사·리뷰

cargo-llvm-cov0.9.1, core source 전체, tests/examples 제외: line1491/1536=97.0703%, function131/135=97.0370%, region2322/2423=95.8316%. branch coverage는 측정하지 않았다. [측정 요약](lie-coverage-summary.json)을 보존한다. 미사용 generic proof getter를 수치 목적으로 제외하지 않았다. 전체 애플리케이션80%와 별도다.

fmt/clippy·workspace tests·WASM target check·공개 TS freshness·문서/Mermaid 검사 통과. Windows 샌드박스에서 Vite subprocess EPERM이 발생해 웹 검증은 일반 환경에서 재실행했다. 실제 DB는 PostgreSQL CI에서 확인하며 로컬 DB 미실행을 통과로 집계하지 않는다. PR의 필수6개 checks와 최신 head를 확인하고 위임에 따라 병합한다.

리뷰에서 부분 모델 결과를 확정으로 사용하지 않는지, 숨은 Board 없이 proof를 만드는지, 기존 history 전체에 certificate를 묶는지, 두 lie/capacity/0/범위/모호성/예산 거절과 공개 이력만의 재접속 경계를 확인했다. 공정성의 수학적 안전성 검증을 사람의 이해·재미·실기기 용량 결과로 대신하지 않는다.
