# #168 · WASM 도구와 라이브러리 스키마 정합성

NFR01/05/08 · TS02/22/30 · [ADR0013](../adr/0013-public-wasm-and-native-contract.md).

사용자가 최신 wasm-bindgen CLI0.2.130을 설치한 뒤 실제 wasm:build에서 저장된0.2.129와 schema mismatch로 실패했다. 사용자 실행 Red와 별도로 develop lock0.2.129/설치 CLI0.2.130의 불일치를 읽기 전용으로 재확인했다. 사용자 cargo update/wasm:generate 결과는 별도 복사해 보존한다. 버전 인자 없는 cargo add로 최신 안정 범위를 선언하고 실제 빌드·생성 선언·native/WASM fixture·관련 브라우저 결과를 확인한다. 관련 없는 Windows lock 참조 선택은 이 변경에 섞지 않는다. 실행 결과와 남은 한계는 검증 뒤 기록한다.


실행 결과: 버전 인자 없는 cargo add는 기존0.2.129 호환 범위를 유지했다. 이 범위는0.2.130도 허용하므로 manifest를 임의 수정하지 않고 실제 lock의 관련7개 패키지와 도구 생성 선언을 함께 반영한다. 사용자 Windows lock 참조6곳은 그대로 작업 트리에 남겨 이 변경에서 제외한다. wasm:build와 전체 production build, fixtures:check, cargo check --locked -p liar-wasm --target wasm32-unknown-unknown, 전체 typecheck가 통과했다. 실제 WASM PC/mobile 튜토리얼·3난이도 연습12개를 재시도 없이 확인했고8언어/360px 추가4개를 확인했다. 단위366개/line91.56%를 확인했다. 이 로컬 검증 당시 별도 #167 UI 변경이 작업 트리에 있었으며 이 PR에는 포함하지 않는다. CI의 깨끗한 checkout 검사는 별도로 확인한다.

코드 리뷰: core/protocol/게임 규칙과 수동 생성 타입 수정은 없다. 생성 선언의 externref 표/내부 export 변화는0.2.130 도구의 실제 산출물이며 최신 도구와 저장된 라이브러리의 스키마가 일치한다. 사용자 값·비밀·테스트 산출물은 커밋에 넣지 않는다. target_features 경고는 기존 strip 설정에서 계속 출력되며 이번 스키마 정합성 수정으로 해결했다고 주장하지 않는다. 외부 OAuth·운영 성능·실제 사람의 이해는 검증하지 않았다.
