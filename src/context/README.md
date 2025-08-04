## Editor Context

- 데이터 편집을 임시본을 하위 요소들과 효과적으로 공유하기 위함

### Context Provider

```tsx
<EditorProvider
  key={'some-unique-key'}
  value={initialValue}
  onFinalize={saveValue}
>
  ...
</EditorProvider>
```

### Usage Pattern

```tsx
...
const {update, commit} = useEditor("some-unique-key");

update(prev => {...prev, prop: value}); // update

commit(); // save value and will cause onFinalize callback called with final value as parameter

```

## Chat Context

- 채팅 메시지 상태와 스트리밍을 관리하고 여러 컴포넌트 간에 공유

### Context Provider

```tsx
<ChatProvider>
  <Chat>
    <Chat.Messages />
    <Chat.Input />
  </Chat>
</ChatProvider>
```

### Usage Pattern

```tsx
const { messages, isLoading, submit } = useChatContext();

// 메시지 전송
await submit([userMessage]);

// 메시지 렌더링
messages.map((message) => <MessageBubble key={message.id} message={message} />);
```
