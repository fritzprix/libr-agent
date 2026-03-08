# Issues

## 1. SubAgent 실행 오류 (404) 시 misleading SuccessHint

```json
[
  {
    "id": "msg_1772900054511",
    "sessionId": "sc2gvxtucqntq2g3m62xtftt",
    "threadId": "sc2gvxtucqntq2g3m62xtftt",
    "role": "assistant",
    "content": [
      {
        "type": "tool_call",
        "id": "tool_mkpzhj41mekylyq7556rgp1f",
        "name": "swarm__spawnAgent",
        "arguments": "{\"assistantId\":\"coding-expert\",\"request\":\"Please collect historical daily stock price data (Close, Volume, High, Low) for the 'Magnificent 7' stocks: AAPL, MSFT, NVDA, GOOGL, AMZN, META, TSLA for the last 3 years. \\nCheck if 'yfinance', 'pandas', 'matplotlib', and 'seaborn' are available. If not, try to install them or suggest an alternative. \\nSave the collected data as CSV files in the workspace. Then, provide a brief summary table of their performance (Total Return, Annualized Volatility) for the period.\"}"
      }
    ],
    "tool_calls": [
      {
        "id": "tool_mkpzhj41mekylyq7556rgp1f",
        "type": "function",
        "function": {
          "name": "swarm__spawnAgent",
          "arguments": "{\"assistantId\":\"coding-expert\",\"request\":\"Please collect historical daily stock price data (Close, Volume, High, Low) for the 'Magnificent 7' stocks: AAPL, MSFT, NVDA, GOOGL, AMZN, META, TSLA for the last 3 years. \\nCheck if 'yfinance', 'pandas', 'matplotlib', and 'seaborn' are available. If not, try to install them or suggest an alternative. \\nSave the collected data as CSV files in the workspace. Then, provide a brief summary table of their performance (Total Return, Annualized Volatility) for the period.\"}"
        }
      }
    ],
    "tool_call_id": null,
    "isStreaming": false,
    "thinking": null,
    "assistantId": null,
    "attachments": null,
    "tool_use": null,
    "createdAt": "2026-03-07T16:14:18.134Z",
    "updatedAt": "2026-03-07T16:14:18.134Z",
    "source": null,
    "error": null,
    "metadata": null
  },
  {
    "id": "1dc3405a-0cf5-4ef1-9e5a-d6cbdd846624",
    "sessionId": "sc2gvxtucqntq2g3m62xtftt",
    "threadId": "sc2gvxtucqntq2g3m62xtftt",
    "role": "tool",
    "content": [
      {
        "type": "text",
        "text": "✗ Request failed (404): Assistant not found: coding-expert\n\n💡 Next Steps:\n1. Verify the agent ID is correct using getChildAgents\n2. The session may have been terminated — use getChildAgents to list active sessions\n3. Use spawnAgent to create a new agent if needed",
        "isError": true
      }
    ],
    "tool_calls": null,
    "tool_call_id": "tool_mkpzhj41mekylyq7556rgp1f",
    "isStreaming": false,
    "thinking": null,
    "thinkingSignature": null,
    "assistantId": null,
    "attachments": null,
    "tool_use": null,
    "createdAt": "2026-03-07T16:14:18.375Z",
    "updatedAt": "2026-03-07T16:14:18.375Z",
    "source": "tool",
    "error": null,
    "metadata": {
      "toolError": true
    }
  }
]
```

## 2. Knowledge 도구 Session 간 공유 불가 이슈

- 상세: Knowledge assistants 및 session간 공유 불가
- 기대: session간 assistant간 공유 가능해야함
