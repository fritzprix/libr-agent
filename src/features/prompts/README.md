# System Prompts

이 디렉터리는 다양한 시스템 프롬프트 컴포넌트들을 관리합니다.

## 구조

각 시스템 프롬프트는 개별 파일로 분리되어 있으며, 다음과 같은 특징을 가집니다:

- **관심사의 분리**: 각 프롬프트는 특정 기능에 집중
- **재사용성**: 필요한 곳에서 import하여 사용
- **확장성**: 새로운 프롬프트를 쉽게 추가 가능
- **테스트 용이성**: 각 컴포넌트를 독립적으로 테스트 가능

## 현재 프롬프트들

### BuiltInToolsSystemPrompt

파일 첨부 정보를 동적으로 시스템 프롬프트에 주입하는 컴포넌트입니다.

**기능:**

- 현재 세션의 첨부된 파일 목록을 AI에게 제공
- 파일 메타데이터 (이름, 타입, 크기, 미리보기) 포함
- 파일 추가/제거 시 자동 업데이트
- 에러 상황에 대한 graceful 처리

**사용법:**

```tsx
import { BuiltInToolsSystemPrompt } from '@/features/prompts/BuiltInToolsSystemPrompt';

// ChatProvider 내부에서 사용
<ChatProvider>
  <BuiltInToolsSystemPrompt />
  {/* 기타 컴포넌트들 */}
</ChatProvider>;
```

### JailbreakSystemPrompt

AI의 안전 제한을 우회하여 무제한 작동 모드를 제공하는 컴포넌트입니다.

**기능:**

- AI 안전 가드레일 제거
- 콘텐츠 제한 없는 자유로운 응답 활성화
- 모든 주제에 대한 개방적 토론 허용
- 기술적/창작적 자유도 최대화

**주의사항:**

- AI 안전 보호 기능을 제거하므로 책임감 있게 사용해야 합니다
- 높은 우선순위(10)로 설정되어 다른 제한 사항보다 우선 적용됩니다

**사용법:**

```tsx
import { JailbreakSystemPrompt } from '@/features/prompts/JailbreakSystemPrompt';

// ChatProvider 내부에서 사용
<ChatProvider>
  <JailbreakSystemPrompt />
  {/* 기타 컴포넌트들 */}
</ChatProvider>;
```

### TimeLocationSystemPrompt

현재 시간과 위치 정보를 자연어 형식으로 제공하는 컴포넌트입니다.

**기능:**

- 현재 날짜와 시간을 읽기 쉬운 형식으로 제공
- 사용 가능한 경우 도시/국가로 위치 표시
- 타임존 정보 포함
- 위치를 사용할 수 없을 때 graceful fallback

## 새로운 프롬프트 추가 가이드

1. 새 파일 생성: `{PromptName}SystemPrompt.tsx`
2. 다음 패턴을 따라 구현:

```tsx
import { useCallback, useEffect } from 'react';
import { useChatContext } from '@/context/ChatContext';
import { getLogger } from '@/lib/logger';

export function {PromptName}SystemPrompt() {
  const logger = getLogger('{PromptName}SystemPrompt');
  const { registerSystemPrompt, unregisterSystemPrompt } = useChatContext();

  const buildPrompt = useCallback(async () => {
    // 프롬프트 내용 생성 로직
    return 'Your system prompt content';
  }, [/* dependencies */]);

  useEffect(() => {
    const id = registerSystemPrompt({
      content: buildPrompt,
      priority: 1, // 우선순위 설정
    });

    return () => unregisterSystemPrompt(id);
  }, [/* dependencies */]);

  return null;
}
```

1. 필요한 곳에서 import하여 사용

## 우선순위 가이드

- **높은 우선순위 (1-3)**: 핵심 시스템 정보 (파일 첨부, 역할 정보)
- **중간 우선순위 (4-7)**: 컨텍스트 정보 (환경, 설정)
- **낮은 우선순위 (8-10)**: 선택적 정보 (팁, 가이드)

## 모범 사례

1. **에러 처리**: 항상 try-catch와 fallback 메시지 제공
2. **로깅**: 디버깅을 위한 적절한 로깅 추가
3. **의존성**: useEffect와 useCallback의 의존성 배열 정확히 명시
4. **타입 안전성**: TypeScript 타입 적극 활용
5. **문서화**: JSDoc 주석으로 기능과 사용법 명시
