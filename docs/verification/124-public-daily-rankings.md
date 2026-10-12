# #124 최소 정보의 공개 데일리 순위

[이슈124](https://github.com/crystal23733/search-mine/issues/124) · [ADR0058](../adr/0058-public-daily-rankings.md) · M3/상위19.

#122/PR123 develop1414cda22708b13af1207f98215069b89edc2381 병합·#122 CLOSED를 확인하고 설계·추적표부터 갱신했다.

## 동작과 리뷰

고정 public POST rankings는 쿠키·로그인·CSRF·계정 생성 없이 같은 origin에서 읽는다. Rust closed 입력/응답과 생성TS를 원천으로 사용한다. accepted metadata와 각50개/has_more를 제공한다. 시간 순위는 저장된 Some만 서버 시간 우선, 완료 순위는 NULL도 포함하여 오류 우선이다. current nickname·position·mistakes와 time의 elapsed_ms만 공개하고 client replay 시간·타인의 ID/replay/digest/subject/token은 제외한다. 전체 COUNT/임의offset/50위 밖 개인 등수는 제공하지 않는다.

별도 DailyRankingReader·serializable하지 않는 snapshot, PG repeatable-read/read-only transaction·current nickname JOIN·LIMIT51·기존 publication/receipt validator·forward index202610120002를 사용한다. 정확90일 만료와 손상을 검사한다. 삭제·닉네임 변경은 후속 조회에 반영하며 이미 전달한 응답을 회수한다고 주장하지 않는다. requests2/global20초당/HTTP2초·입력4KiB/응답64KiB·no-store, 기존 ResultRate의 고정 budget key로 방문자/IP 추적을 피한다. main/HTTPS fixture의 공개·인증 API는 같은 CPU2 예산 clone을 공유한다.

pm-ai-shipping:code-review correctness/changes/baseline1414cda를 순차 적용했다. accepted rules와 bundled rules를 갈라 projection이 authority를 따르는지 검사했고, 두 Native worker가 실제 살아 있는 동안 공개 decode의503을 강제했다. reader의 미게시 날짜 조기 None가 clock 재확인을 생략하는 경계를 actual PG Red0pass1fail/0.47초로 확인해 Some/None 공통 재관측·commit 경로로 수정했다. HTTP outer observe는 이 역행을 이미 거절했으며 port 계약도 일치시켰다. 검토한 범위에서 추가로 근거가 뒷받침된 결함은 없다. 독립 보안·성능 감사나 인간·RTT 검증은 수행하지 않았다.

## 실제 실행과 준비 오류

최초 Red 필터는 --exact와 짧은 이름이 맞지 않아0개였다. 로그를 보존하고 전체 이름으로 실제404vs200/0pass1fail/0.00초를 확인한 뒤 Green200을 실행했다. 순수 projection3/0.01초·HTTP17/5.23초·초기 actualPG 새4/2.97초, 이후 clock 회귀를 추가했다. 보조 테스트 파일 위치·괄호 구문·backlog root key 가정 오류는 수정 후 재실행했으며 행동 Red와 구분한다. 완료122 generated coverage cache만 정확 workspace/nonreparse·MERGED·성공JSON/로그 보존을 확인하여 정리했다. 기존 DB/data/checksum·선행 migration·원문/main/review는 보존했다. 새 forward migration은 public DB 적용 전에 LF로 고정하고 actual migrate0을 확인했다.

PC1440/mobile390 HTTPS는 actualWASM·서버 시간 PG 저장·쿠키/CSRF 없는 순위 읽기·최소필드/ID제외·로그아웃 이후 공개200·기존 인증 제출401·8언어 본인 export를 검사했다. 주입 clock은 인간 풀이·벽시계·사전 계산 방지·글로벌 RTT 증거가 아니다.

최초 전체 브라우저 실행은 이전 #122 기록이 남은 DB를 재사용하여 84통과/1실패/17미실행/3.4분이었다. 새 fixture는 시계를1800000000부터 시작하지만 기존 완료 기록은1800006737까지 남아 있었다. 공개 순위의 미래 기록 검사는503으로 닫혔다. 실패 로그와 기존 DB를 보존하고 새 전용 DB liar_daily124에 마이그레이션을 적용한 뒤 전체 검사를 재실행해 통과했다. 미래 시간 검사·503·재시도 설정은 완화하지 않았다. 보조 실행기의 복사 경로·PowerShell stderr 처리·MIGRATION_DATABASE_URL 누락도 수정했으며 제품 테스트 통과로 세지 않는다.

## 최종 로컬 검증

최종 scripts/check.ps1 실제 exit0: Rust 395개, 웹 221개/38파일/line88.85%, Chromium 102개/3.7m/재시도0. 실제 PostgreSQL17.4 회귀101개/14.60초, fresh 계측101개/15.03초. Daily 877/893 = 98.21%, Privacy 164/164 = 100.00%, 서버 전체 9090/9700 = 93.71%. 도메인95%/전체80% 기준 충족. 문서174개/시나리오36개/Mermaid34개 실패0.

원격 PR/CI·정확head 병합·#124 CLOSED는 다음 확인 단계다. 명시 공식 웹/대기 제출·상위19 통합·외부42·운영25/26·사람27·미확정83은 미완료다.
