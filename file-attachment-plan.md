# 파일 첨부 기능 통합 계획

## 🎯 목표

ResourceAttachmentContext와 file-store MCP 서버를 Chat 시스템에 통합하여 기본 파일 첨부 기능 구현

## 📋 현재 상황 분석

### ✅ 이미 구현된 것들

1. **ResourceAttachmentContext.tsx** - file-store MCP 서버와 연동된 상태 관리
2. **file-store.ts** - MCP 파일 저장/검색 서버 (TextChunker, BM25SearchEngine 포함)
3. **Message 모델** - `attachments?: AttachmentReference[]` 필드 존재
4. **AttachmentReference 인터페이스** - 완전히 정의됨

### ❌ 구현 필요한 것들

1. **MessageBubble.tsx** - 첨부 파일 표시 로직 없음
2. **Chat.tsx** - ResourceAttachmentContext 연동 없음
3. **use-ai-service.ts** - 첨부 파일을 AI 시스템 메시지에 포함하는 로직 없음

## 🔧 파일 분리 계획

### 현재 file-store.ts의 문제

하나의 파일에 너무 많은 책임이 집중:

- 파일 저장/관리 (MCP 서버)
- 텍스트 청킹 (TextChunker 클래스)
- 텍스트 검색 (BM25SearchEngine)

### 분리 방향

```typescript
// 기존
src/lib/web-mcp/modules/file-store.ts  // 모든 것이 여기

// 분리 후
src/lib/web-mcp/modules/file-store.ts  // 파일 저장/관리 + BM25SearchEngine만
src/lib/file-processing/
├── text-chunker.ts                    // TextChunker 클래스 이동
└── text-extractor.ts                  // File → 텍스트 추출 (새로 구현)
```

**주의**: 검색 엔진은 분리하지 않고 file-store.ts에 그대로 둠

## 📝 구현 작업 목록

### 1. MessageBubble.tsx 수정

첨부 파일 표시 로직 추가:

```tsx
// message.attachments가 있으면 파일 정보 표시
{
  message.attachments && message.attachments.length > 0 && (
    <div className="mb-3 p-3 bg-muted/30 rounded-lg">
      <div className="text-sm mb-2">📎 {message.attachments.length}개 파일</div>
      {message.attachments.map((attachment) => (
        <div key={attachment.contentId} className="text-xs">
          📄 {attachment.filename} ({attachment.size} bytes)
        </div>
      ))}
    </div>
  );
}
```

### 2. Chat.tsx 수정

ResourceAttachmentContext 연동:

```tsx
// useResourceAttachment 훅 사용
const { addFile, files, clearFiles, removeFile } = useResourceAttachment();

// 파일 선택 시 MCP 서버에 저장
const handleFileAttachment = async (e: React.ChangeEvent<HTMLInputElement>) => {
  const fileList = e.target.files;
  if (!fileList) return;

  for (const file of fileList) {
    await addFile(file); // MCP 서버에 실제 저장
  }
};

// 메시지 전송 시 첨부 파일 포함
const handleSubmit = async () => {
  const userMessage: Message = {
    // ...기존 필드들...
    attachments: files.length > 0 ? [...files] : undefined,
  };

  clearFiles(); // 전송 후 첨부 상태 초기화
  await submit([userMessage]);
};
```

### 3. use-ai-service.ts 수정

첨부 파일 내용을 AI 시스템 메시지에 포함:

```tsx
const extractAttachmentContext = async (
  messages: Message[],
): Promise<string> => {
  if (!fileStore) return '';

  const attachmentContents: string[] = [];

  for (const message of messages) {
    if (message.attachments?.length) {
      for (const attachment of message.attachments) {
        const content = await fileStore.retrieve_file({
          id: attachment.contentId,
        });
        attachmentContents.push(
          `--- ${attachment.filename} ---\n${content.content}`,
        );
      }
    }
  }

  return attachmentContents.length > 0
    ? `\n첨부된 파일들:\n\n${attachmentContents.join('\n\n')}\n`
    : '';
};

const submit = async (messages: Message[]) => {
  const attachmentContext = await extractAttachmentContext(messages);

  const stream = serviceInstance.streamChat(messages, {
    systemPrompt: [
      getCurrentAssistant()?.systemPrompt || DEFAULT_SYSTEM_PROMPT,
      attachmentContext, // 첨부 파일 내용 추가
    ]
      .filter(Boolean)
      .join('\n\n'),
  });
};
```

### 4. TextChunker 분리

file-store.ts에서 TextChunker 클래스를 별도 파일로 이동:

```typescript
// src/lib/file-processing/text-chunker.ts
export class TextChunker {
  constructor(private chunkSize: number = 500) {}

  chunkText(text: string): string[] {
    // 기존 구현 그대로 이동
  }
}
```

### 5. 텍스트 추출기 구현

파일에서 텍스트를 추출하는 유틸리티:

```typescript
// src/lib/file-processing/text-extractor.ts
export const extractTextFromFile = async (file: File): Promise<string> => {
  if (file.type.startsWith('text/')) {
    return await file.text();
  }

  // PDF 등 다른 타입은 향후 구현
  throw new Error(`Unsupported file type: ${file.type}`);
};
```

## 🗂️ 최종 파일 구조

```
src/
├── features/chat/
│   ├── Chat.tsx                          # ResourceAttachmentContext 연동
│   └── MessageBubble.tsx                 # 첨부 파일 표시 추가
├── context/
│   └── ResourceAttachmentContext.tsx     # 기존 유지
├── lib/
│   ├── web-mcp/modules/
│   │   └── file-store.ts                # 파일 저장/관리 + BM25검색
│   └── file-processing/
│       ├── text-chunker.ts              # TextChunker 이동
│       └── text-extractor.ts            # 파일 텍스트 추출
└── hooks/
    └── use-ai-service.ts                 # 첨부 파일 처리 추가
```

## 📋 구현 순서

1. **TextChunker 분리** - file-store.ts → text-chunker.ts
2. **text-extractor.ts 구현** - 파일 텍스트 추출 유틸리티
3. **MessageBubble.tsx 수정** - 첨부 파일 표시
4. **Chat.tsx 수정** - ResourceAttachmentContext 연동
5. **use-ai-service.ts 수정** - 첨부 파일을 AI 컨텍스트에 포함

## ⚠️ 주의사항

1. **파일 크기 제한** - AI 토큰 제한 고려
2. **에러 처리** - MCP 서버 연결 실패, 파일 읽기 실패 등
3. **성능** - 대용량 파일 처리 시 UI 블로킹 방지
4. **타입 안전성** - 모든 연동 부분에서 타입 체크

## 🎯 예상 결과

- 사용자가 파일을 첨부하면 file-store MCP 서버에 저장
- 메시지 전송 시 첨부 파일 정보가 포함
- AI가 첨부 파일 내용을 참조하여 응답
- 메시지에서 첨부 파일 정보 표시
