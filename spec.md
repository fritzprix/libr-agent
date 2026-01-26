# Spec

## 메시지 버블 표시 관련 Spec

- Think (Reasoning) 메시지는 Streaming 중 Bubble에 표시가 되어 사용자의 기다림의 불편함을 최소화해야 함
- content와 도구 사용이 없이 Think만 포함되어 있는 메시지 허용
  - 이러한 메시지는 빈 메시지로 오류로 간주하지 않아야 하며 정상 메시지로 추가되어야함
  - 또한 정상적인 메시지 버블로 표시가 되어야 함
- Streaming이 종료되고 Think만 포함된 메시지가 Message Stack에 추가되면 이것은 추가적인 Agent Turn을 요청해야하는 것으로 간주, 재차 llm request를 보낸다. (도구 사용 시 Recurring Request 메커니즘 참고)

## AgentChatStatusBar의 agent mode toggle의 역할

- Agent Mode가 On일 때는 Tool Call을 강제하도록 LLMServiceContext 및 각 LLMService 구현체에 파라미터가 적용되어야 함
- 이를 통해 Tool Call의 지속적인 Recurring Pattern으로 동작하도록 되어햐 함
