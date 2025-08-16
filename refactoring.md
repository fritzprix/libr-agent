# Refactoring Plan — Chat.tsx의 UI 디자인 개선 및 기능 오류 수정

## 1. Icon 개선 - Lucide React 아이콘 적용

### 현재 문제점

// 드래그 오버레이 스타일링

<div className={cn(
  "transition-colors",
  dragState === 'valid' && "bg-green-500/10 border-green-500",
  dragState === 'invalid' && "bg-destructive/10 border-destructive"
)}>
```

### 현재 문제점

- Send/Cancel 버튼: `{isLoading ? '✕' : '⏎'}` (텍스트 기반, 작고 조잡함)
- 첨부 파일 아이콘: `📎` (이모지 기반, 일관성 없음)
- 삭제 버튼: `✕` (텍스트 기반, 작음)

### 수정 파일들

- `/src/features/chat/Chat.tsx` (라인 724-733: Send/Cancel 버튼)
- `/src/components/ui/FileAttachment.tsx` (라인 49, 75, 115: 📎, ✕ 아이콘)

### 변경 방향

```tsx
// 기존
{
  isLoading ? '✕' : '⏎';
}

// 개선 후 - shadcn/ui Button 활용
import { Send, X } from 'lucide-react';
import { Button } from '@/components/ui/button';

<Button
  type="submit"
  variant="ghost"
  size="icon"
  disabled={!isLoading && !input.trim() && attachedFiles.length === 0}
  title={isLoading ? 'Cancel request' : 'Send message'}
>
  {isLoading ? <X /> : <Send />}
</Button>;
```

```tsx
// 첨부 파일 아이콘 개선 - shadcn/ui Button 활용
import { Paperclip, Trash2 } from 'lucide-react';
import { Button } from '@/components/ui/button';

// 첨부 버튼
<Button variant="ghost" size="sm" type="button" onClick={handleFileSelect}>
  <Paperclip />
</Button>

// 삭제 버튼
<Button variant="ghost" size="icon" onClick={() => onRemove(index)}>
  <Trash2 className="text-destructive" />
</Button>
```

## 2. Drag & Drop 기능 오류 수정

### 현재 문제 분석

- **Drop 이벤트 중복 호출**: 로그 분석 결과 drop 이벤트가 2번 연속 호출되어 같은 파일이 중복 추가되려는 문제 발생
- 첫 번째 drop에서 파일 추가 성공, 두 번째 drop에서 "already being uploaded" 에러 발생
- `/src/features/chat/Chat.tsx` 라인 452-460: `onDragDropEvent` 리스너에서 중복 처리 방지 로직 부재

### 현재 코드 상태

```tsx
unlisten = await webview.onDragDropEvent((event) => {
  if (event.payload.type === 'enter') {
    setIsDragOver(true);
  } else if (event.payload.type === 'drop') {
    // 이 부분만 파일 처리해야 함
    setIsDragOver(false);
    handleFileDrop(event.payload.paths);
  } else if (event.payload.type === 'leave') {
    setIsDragOver(false);
  }
});
```

### 개선 방향

1. **Drop 이벤트 중복 방지 로직 추가**
   - `handleFileDrop` 함수에 debounce 또는 중복 호출 방지 메커니즘 구현
   - 동일한 파일 경로의 연속 drop 이벤트를 필터링

2. **파일 지원 여부 검사 로직 추가**
   - `handleFileDrop` 함수 내에서 파일 확장자 검사를 `enter` 이벤트 시에도 수행
   - 지원되지 않는 파일은 시각적으로 거부 표시

3. **시각적 피드백 개선**
   - `isDragOver` 상태를 세분화: `dragValid`, `dragInvalid`
   - CSS 클래스로 지원/미지원 파일 구분 표시

```tsx
// 개선된 드래그 상태 관리
const [dragState, setDragState] = useState<'none' | 'valid' | 'invalid'>(
  'none',
);
const dropTimeoutRef = useRef<NodeJS.Timeout>();

// 파일 검증 함수 분리
const validateFiles = (filePaths: string[]) => {
  const supportedExtensions = /\.(txt|md|json|pdf|docx|xlsx)$/i;
  return filePaths.every((path) => {
    const filename = path.split('/').pop() || path.split('\\').pop() || '';
    return supportedExtensions.test(filename);
  });
};

// Drop 중복 방지 처리
const handleFileDrop = useCallback((filePaths: string[]) => {
  // 기존 타이머 클리어
  if (dropTimeoutRef.current) {
    clearTimeout(dropTimeoutRef.current);
  }

  // 짧은 지연 후 실행 (중복 이벤트 방지)
  dropTimeoutRef.current = setTimeout(() => {
    // 실제 파일 처리 로직
    processFileDrop(filePaths);
  }, 10);
}, []);
```

## 3. 첨부 파일 리스트에 삭제 버튼 개선

### 현재 상태

- `/src/components/ui/FileAttachment.tsx` 라인 108-119: 이미 삭제 버튼 존재
- 하지만 아이콘이 텍스트 기반 `✕`으로 작고 눈에 잘 안 띔

### 구현 방향

1. **아이콘 개선**: shadcn/ui Button 컴포넌트와 Lucide React 아이콘 활용
2. **버튼 variants 적용**: `variant="ghost"`, `size="icon"` 사용으로 일관된 스타일링
3. **접근성 강화**: shadcn/ui의 기본 접근성 기능 활용

```tsx
// 개선된 삭제 버튼 - shadcn/ui 방식
import { Trash2 } from 'lucide-react';
import { Button } from '@/components/ui/button';

<Button
  type="button"
  variant="ghost"
  size="icon"
  onClick={() => onRemove(index)}
  title="Remove file"
>
  <Trash2 className="text-destructive" />
</Button>;
```

## 4. 구현 우선순위

1. **1단계**: Lucide React 아이콘 적용 (Send, Cancel, Paperclip, Trash)
2. **2단계**: Drag & Drop 파일 검증 로직 개선
3. **3단계**: 시각적 드래그 피드백 개선 (CSS 클래스, 상태 관리)
4. **4단계**: 첨부 파일 삭제 버튼 UX 개선

## 5. 테스트 케이스

- [ ] Send/Cancel 버튼 아이콘이 명확하게 표시되는지
- [ ] 지원되는 파일 드래그 시 초록색 표시
- [ ] 지원되지 않는 파일 드래그 시 빨간색 표시
- [ ] Drop 이벤트에서만 파일이 실제 추가되는지
- [ ] 첨부 파일 삭제 버튼 클릭 영역이 충분한지
