# Window에서 execute_windows_cmd 사용시 오류

## 도구 사용

```tool_call
[
  {
    "id": "tool_kx4heavka2ukr43l6qkws5dk",
    "function": {
      "name": "builtin_workspace__execute_windows_cmd",
      "arguments": "{\"command\":\"python pdf_to_text.py 'Isola slides_GenAI 1025.pdf' 'Isola_slides_GenAI_1025.txt'\"}"
    }
  }
]
```

## 도구 응답

```tool_response

[
  {
    "text": "Command failed with exit code 1:\nSTDOUT:\n\n\nSTDERR:\nUsage: python pdf_to_text.py <input_pdf_path> <output_txt_path>",
    "type": "text",
    "serviceInfo": {
      "serverName": "builtin_workspace",
      "toolName": "execute_windows_cmd",
      "backendType": "BuiltInRust"
    }
  }
]

```

## 생각하는 기대 결과

AI Agent가 2개의 args를 지정했으므로 정상적으로 실행이 되어야 함
