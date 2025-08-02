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
