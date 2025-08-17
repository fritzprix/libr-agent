# Refactoring Plan: MCP Tool Response JSON 파싱 경고 해결

## 작업의 목적

AI 서비스에서 MCP Tool Response를 처리할 때 발생하는 JSON 파싱 경고를 해결한다. MCP 스펙에 따라 Tool Response는 JSON 형태가 아닐 수 있음에도 불구하고 강제로 JSON으로 파싱하려고 시도하여 경고가 발생하고 있다. MCP 스펙을 준수하면서 다양한 형태의 Tool Response (JSON, 텍스트, 에러 메시지)를 올바르게 처리하도록 개선한다.

## 현재의 상태 / 문제점

### 1. JSON 파싱 강제 시도로 인한 경고

```log
[2025-08-17][04:11:54][webview][WARN] [AIService] Tool message content is not valid JSON, wrapping as object: Error: Mcp error: -32602: MCP error -32602: Invalid arguments for tool manipulateCube: [] (Code: -32603)
```

- AI 서비스에서 모든 Tool Response를 JSON으로 파싱하려고 시도
- MCP 에러 응답이나 일반 텍스트 응답도 강제로 JSON 변환 시도
- 파싱 실패 시 경고 로그 발생하며 객체로 래핑

### 2. MCP 스펙 비준수

**MCP 스펙에서 정의된 ContentBlock 타입들:**

- `TextContent`: 일반 텍스트 (JSON이 아님)
- `ImageContent`: Base64 인코딩된 이미지
- `AudioContent`: Base64 인코딩된 오디오  
- `ResourceLink`: 리소스 링크
- `EmbeddedResource`: 임베디드 리소스

**현재 코드의 문제:**

```typescript
// 모든 응답을 JSON으로 파싱 시도
const parsedChunk = JSON.parse(chunk);  // 에러 발생 가능
```

- MCP Tool Response는 항상 JSON 형태가 아닐 수 있음
- 에러 메시지나 일반 텍스트도 유효한 응답임에도 파싱 실패로 처리

### 3. 부적절한 에러 응답 처리

**MCP 에러 응답 예시:**

```text
"Error: Mcp error: -32602: MCP error -32602: Invalid arguments for tool manipulateCube"
```

- 에러 메시지가 일반 텍스트 형태로 전달됨
- 현재는 JSON 파싱 실패로 처리하여 부적절한 경고 발생
- MCP 에러 코드와 메시지 정보가 제대로 추출되지 않음

## 변경 이후의 상태 / 해결 판정 기준

### 성공 기준

1. Tool Response JSON 파싱 경고 완전 제거
2. MCP 스펙에 따른 올바른 ContentBlock 타입 처리
3. JSON, 텍스트, 에러 메시지 등 모든 형태의 Tool Response 적절히 처리
4. MCP 에러 응답의 구조화된 정보 추출 및 처리

### 검증 방법

1. 로그에서 "Tool message content is not valid JSON" 경고 사라짐
2. 다양한 Tool Response 타입에 대해 적절한 처리 확인
3. MCP 에러 발생 시 구조화된 에러 정보 제공
4. Tool Response 처리 과정의 명확한 로깅

## 수정이 필요한 코드 및 수정부분의 코드 스니핏

### 1. `src/hooks/use-ai-service.ts` - 스트림 청크 파싱 개선

**현재 코드:**

```typescript
for await (const chunk of stream) {
  const parsedChunk = JSON.parse(chunk);
  
  if (parsedChunk.thinking) {
    thinking += parsedChunk.thinking;
  }
  
  if (parsedChunk.tool_calls) {
    toolCalls = parsedChunk.tool_calls || [];
  }
  
  if (parsedChunk.content) {
    fullContent += parsedChunk.content;
  }
  
  // Update streaming state
  setResponse({
    id: currentResponseId,
    content: fullContent,
    thinking,
    role: 'assistant',
    isStreaming: true,
    tool_calls: toolCalls,
    sessionId: messages[0]?.sessionId || '',
  });
}
```

**수정 후 코드:**

```typescript
for await (const chunk of stream) {
  let parsedChunk: any;
  
  try {
    parsedChunk = JSON.parse(chunk);
  } catch (parseError) {
    // Handle non-JSON chunks (e.g., plain text tool responses)
    logger.debug('Received non-JSON chunk, treating as text content', { 
      chunk: chunk.substring(0, 100) + '...',
      chunkType: typeof chunk 
    });
    parsedChunk = { content: chunk };
  }
  
  if (parsedChunk.thinking) {
    thinking += parsedChunk.thinking;
  }
  
  if (parsedChunk.tool_calls) {
    toolCalls = parsedChunk.tool_calls || [];
  }
  
  if (parsedChunk.content) {
    fullContent += parsedChunk.content;
  }
  
  // Update streaming state
  setResponse({
    id: currentResponseId,
    content: fullContent,
    thinking,
    role: 'assistant',
    isStreaming: true,
    tool_calls: toolCalls,
    sessionId: messages[0]?.sessionId || '',
  });
}
```

### 2. `src/lib/ai-service/gemini.ts` - Tool Response 처리 개선

**현재 코드:**

```typescript
private convertToGeminiMessages(messages: Message[]): Content[] {
  // ...existing code...
  
  if (message.role === 'tool') {
    let responseContent: string;
    
    if (message.content) {
      try {
        const parsed = JSON.parse(message.content);
        responseContent = JSON.stringify(parsed);
      } catch {
        logger.warn('Tool message content is not valid JSON, wrapping as object', message.content);
        responseContent = JSON.stringify({ value: message.content });
      }
    } else {
      responseContent = JSON.stringify({ error: 'No content in tool response' });
    }
    
    // ...existing code...
  }
}
```

**수정 후 코드:**

```typescript
private convertToGeminiMessages(messages: Message[]): Content[] {
  // ...existing code...
  
  if (message.role === 'tool') {
    const logger = getLogger('GeminiToolResponse');
    
    // 상세한 디버깅 정보 추가
    logger.debug('Processing tool response', {
      tool_call_id: message.tool_call_id,
      contentType: typeof message.content,
      contentLength: message.content?.length || 0,
      contentPreview: message.content?.substring(0, 100) + '...',
      isJsonLike: message.content?.trim().startsWith('{') || message.content?.trim().startsWith('[')
    });
    
    const responseContent = this.processToolResponseContent(message.content || '');
    
    return {
      role: 'function',
      parts: [
        {
          functionResponse: {
            name: message.tool_call_id || 'unknown',
            response: {
              value: responseContent,
            },
          },
        },
      ],
    };
  }
  
  // ...existing code...
}

/**
 * Process tool response content based on MCP spec
 */
private processToolResponseContent(content: string): string {
  const logger = getLogger('GeminiToolResponse');
  
  // 1. JSON 형태인지 확인
  try {
    const parsed = JSON.parse(content);
    logger.debug('Tool response is valid JSON, using as-is');
    return JSON.stringify(parsed);
  } catch (jsonError) {
    // JSON이 아님 - MCP TextContent로 처리
    logger.debug('Tool response is not JSON, processing as text content');
  }
  
  // 2. MCP Error 패턴 확인
  if (content.includes('Mcp error:') || content.includes('Error:')) {
    logger.debug('Detected MCP error response, extracting error information');
    
    // MCP 에러 정보 추출
    const errorInfo = this.extractMCPErrorInfo(content);
    return JSON.stringify({
      type: 'mcp_error',
      error: errorInfo.message,
      code: errorInfo.code,
      originalText: content
    });
  }
  
  // 3. 일반 텍스트 응답 - MCP TextContent 형태로 래핑
  logger.debug('Treating tool response as plain text content');
  return JSON.stringify({
    type: 'text',
    text: content
  });
}

/**
 * Extract MCP error information from error text
 */
private extractMCPErrorInfo(errorText: string): { message: string; code: number | null } {
  // MCP 에러 패턴 매칭: "Mcp error: -32602: Invalid arguments..."
  const mcpErrorMatch = errorText.match(/Mcp error:\s*(-?\d+):\s*(.+?)(?:\s*\(Code:|$)/);
  
  if (mcpErrorMatch) {
    return {
      code: parseInt(mcpErrorMatch[1], 10),
      message: mcpErrorMatch[2].trim()
    };
  }
  
  // 일반 에러 패턴: "Error: ..."
  const generalErrorMatch = errorText.match(/Error:\s*(.+)/);
  if (generalErrorMatch) {
    return {
      code: null,
      message: generalErrorMatch[1].trim()
    };
  }
  
  // 패턴에 맞지 않는 경우 전체 텍스트를 메시지로 사용
  return {
    code: null,
    message: errorText
  };
}
```

### 3. `src/lib/ai-service/gemini.ts` - 로깅 개선

**추가할 코드:**

```typescript
// GeminiService 클래스 내부에 추가

/**
 * Log tool response processing statistics
 */
private logToolResponseStats(messages: Message[]): void {
  const logger = getLogger('GeminiToolResponseStats');
  
  const toolMessages = messages.filter(m => m.role === 'tool');
  if (toolMessages.length === 0) return;
  
  const stats = {
    totalToolMessages: toolMessages.length,
    jsonResponses: 0,
    textResponses: 0,
    errorResponses: 0,
    emptyResponses: 0
  };
  
  toolMessages.forEach(msg => {
    if (!msg.content) {
      stats.emptyResponses++;
      return;
    }
    
    try {
      JSON.parse(msg.content);
      stats.jsonResponses++;
    } catch {
      if (msg.content.includes('error:') || msg.content.includes('Error:')) {
        stats.errorResponses++;
      } else {
        stats.textResponses++;
      }
    }
  });
  
  logger.info('Tool response processing statistics', stats);
}

// streamChat 메서드 시작 부분에서 호출
async *streamChat(
  messages: Message[],
  options: {
    modelName?: string;
    systemPrompt?: string;
    availableTools?: MCPTool[];
    config?: AIServiceConfig;
  } = {},
): AsyncGenerator<string, void, void> {
  this.validateMessages(messages);
  
  // Log tool response statistics
  this.logToolResponseStats(messages);
  
  // Validate and fix message stack for Gemini's sequencing rules
  const validatedMessages = this.validateGeminiMessageStack(messages);
  
  // ...existing code...
}
```

## 핵심 변경사항 요약

### ✅ MCP 스펙 준수

1. **다양한 ContentBlock 타입 지원**: JSON, 텍스트, 에러 메시지 모두 적절히 처리
2. **MCP 에러 응답 구조화**: 에러 코드와 메시지를 추출하여 구조화된 형태로 처리
3. **타입별 적절한 래핑**: 각 응답 타입에 맞는 적절한 형태로 변환

### ✅ 로깅 및 디버깅 개선

1. **상세한 디버깅 정보**: Tool Response 처리 과정의 모든 단계 로깅
2. **통계 정보 제공**: Tool Response 타입별 통계 정보로 시스템 상태 파악
3. **경고 제거**: 불필요한 JSON 파싱 경고 완전 제거

### ✅ 복잡도 최소화

1. **기존 구조 유지**: 새로운 메서드 추가로 기존 로직 영향 최소화
2. **점진적 개선**: 기존 기능을 해치지 않으면서 점진적으로 개선
3. **명확한 분리**: Tool Response 처리 로직을 별도 메서드로 분리하여 유지보수성 향상
