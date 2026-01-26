# Spec for Message Bubble

## Think / Reasoning의 메시지 표시 Spec

- Think (Reasoning) 메시지는 Streaming 중 Bubble에 표시가 되어 사용자의 기다림의 불편함을 최소화해야 함
- content와 도구 사용이 없이 Think만 포함되어 있는 메시지 허용
  - 이러한 메시지는 빈 메시지로 오류로 간주하지 않아야 하며 정상 메시지로 추가되어야함
  - 또한 정상적인 메시지 버블로 표시가 되어야 함
- Streaming이 종료되고 Think만 포함된 메시지가 Message Stack에 추가되면 이것은 추가적인 Agent Turn을 요청해야하는 것으로 간주, 재차 llm request를 보낸다. (도구 사용 시 Recurring Request 메커니즘 참고)
- Think (Reasoing) streaming이 업데이트 되는 동안 Think Bubble의 내부 Think Text의 Scroll Position은 가장 아래로 유지되어야 함
- Think Token과 Thinking Token이 Streaming 되는 동안 경과된 시간 (초)를 표시해야함 (그리고 이 기록은 Message에 Persistent 하게 관리되어 이후에도 Rendering이 되어야 함)
