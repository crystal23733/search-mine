# #5 결정론 보드·일반 노게스 검증

FR02, NFR04/05 · TS02/03/27. 이 작업은 거짓말 없는 보드 생성 인증이다. 두 숨은 lie의 안전 진행/간파는 #6 출시 게이트이며 아직 검증되지 않았다.

## 실행된 행동 검증

Red에서 인접 숫자0≠1, 안전 추론빈집합≠예상3칸, 모순을 성공으로 반환, 부분 모델 탐색을 성공으로 반환, 생성 후보를 항상 거절하는 실패를 실행 확인했다. Green 이후 총15개 core 테스트에 작은 판18,432관측 전수·32개 임의 seed의 재검증이 포함된다. flag가 zero flood 경로를 막는 fixture는 독립 확인 후 예상 도달 영역을 수정했다.

- 3×3/2mine의36개 지뢰 배치 ×512개 공개 부분집합: 가능한 모든 숨은 배치를 독립 oracle로 열거해 반환된 안전/지뢰의 확실성을 검사, 반례0.
- 직접·부분집합·모델 추론, 공개 mine·전체 mine 수, 모순·모호성, 노드/구성요소/제약 budget 초과, 유효하지 않은 보드/관측/셀 입력.
- 숫자/flood/flag·재열기 중복, 정확한40mine/216safe/zero opening, 정수 RNG golden, rejection sampling 경계, 결정론 seed 재생성, 증거 변조 거절.
- certify는 Board를 관측으로 투영한 뒤 NoGuessSolver에 Observation만 전달한다. 반환된 결론의 실제 안전성 대조는 추론 뒤 수행하며, 추론에 정답을 제공하지 않는다.
- Board/GeneratedBoard/증거는 Serialize 구현이 없고 온라인 public DTO에 추가되지 않았다. NoGuessSolver는 공격 대전/봇용 solver로 사용하지 않는다.

## 10,000 seed release 측정

코드 커밋 `c313703a6e77823a0dc92aa83c3c97c954f9f4f0`, Rust1.99.0 x86_64-pc-windows-msvc, release thin LTO/codegen-units1. AMD Ryzen9 3900X 12core/24thread, 약32GiB RAM, Windows11 Home 10.0.26200. 개발 PC에서 측정했고 미니 PC 사양/부하/글로벌 RTT 측정은 아니다. 측정 중 도구 빌드/검사 작업도 실행되어 완전히 격리된 성능 표본은 아니다.

고정16×16/40, opening0, RNG1/solver1, seed0~9999, 후보상한256, 구성요소22칸·200,000노드·1024제약. [원시 CSV](generation-10000.csv)의 SHA256은 `f9d64c4b3fb6059d37d4a5fb4e48509780e099801e804ec1260b63335f310130`이다.

| 항목 | 실제 결과 |
|---|---:|
| 최종 생성 성공 / 실패 | 10,000 / 0 |
| 검사한 후보 | 17,463 |
| 후보 채택률 | 57.2639% |
| stalled 거절 / budget 거절 | 7,030 / 433 |
| 재시도한 seed | 4,255 (42.55%) |
| 후보 수 p95 / max | 4 / 13 |
| 생성 p50 / p95 | 0.609ms / 46.430ms |
| 최대 | 1,072.795ms (seed5108, 후보6개) |
| 전체 경과 | 99.969초 |

최대 지연과 budget 거절이 있으므로 온라인 요청 경로에서 동기 생성하지 않는다. 준비된 판 풀·bounded worker·revision 확인은 #16에서 구현한다. 미니 PC의100ms 목표, 간파 검증 비용, 공격 채택률은 이번 결과로 달성했다고 판단하지 않는다.

```powershell
cargo run --locked --release -p liar-core --example generate_bench -- 10000 .tmp/generation.csv
```

## 커버리지와 코드 리뷰

cargo-llvm-cov 최신0.9.1 + llvm-tools-preview, `-p liar-core --tests`, tests/examples 파일 제외. source 전체를 측정하고 unreachable/defensive branch를 임의 제외하지 않는다. 정답 기밀성과 전수 반례 검사를 coverage 수치로 대체하지 않는다. CI는 line95%를 검사하고 JSON artifact를7일 보존한다.

실제 source line665/680=97.7941%, function61/61=100%, region1050/1094=95.9781%였다. board/random line100%, generator95.41%, solver97.51%. [측정 요약](core-coverage-summary.json)을 보존하며 branch coverage는 측정하지 않았다. 전체 애플리케이션80%는 이번 core 측정과 별도다.

RNG는 [작성자 공개 소스](https://prng.di.unimi.it/splitmix64.c)의 wrapping 정수 연산을 명시하며 계정 credential 생성용으로 사용하지 않는다. 모델 DFS는 전부 완료한 구성요소만 채택하고 global mine 수와 frontier 밖 칸의 조합을 결합한다. 실패/예산 초과는 판 채택 불가이며 샘플 성능을 결정론 규칙에 넣지 않는다.
