# Debug Plan 작성 가이드

## 필수 요소

- 문제의 발생 경로
- 해결 이후 상태
- 관련 이슈에 대한 웹 검색 쿼리
- 문제의 원인에 대한 가설 (각 가설에 대한 관련 코드 및 경로 포함 필수)
- 추가 분석 및 확인이 필요한 사항
- 예비적으로 `Debugging History Section`을 추가하고, 이곳에 Debugging하면서 시도한 내역들을 기록하도록 명확한 Instruction을 추가하여 작업자가 Debugging 진행하면서 가설 탐색의 과정을 다음과 같은 원칙에 따라 기록하도록 함
  - 가설
  - 변경 상세 내역, 관련 코드 위치, 변경 부분 상세
  - 해결 여부
  - 결과에 대한 분석 및 새로운 탐색 방향 제시
- file 규칙
  - ./docs/history/debug_{yyyyMMdd_hhmm}.md
