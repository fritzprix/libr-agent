import React, {
  useState,
  useCallback,
  useMemo,
  createContext,
  useContext,
  useRef,
  useEffect,
} from 'react';
import { produce, Draft } from 'immer';
import equal from 'fast-deep-equal';

// --- 타입 정의 (Type Definitions) ---

type EditorContextValue<T> = {
  readonly draft: T;
  readonly originalValue: T;
  readonly isLoading: boolean;
  readonly isDirty: boolean;
  readonly errors: readonly string[];
  update: (updater: (draft: Draft<T>) => void) => void;
  commit: () => Promise<void>;
  reset: () => void;
  setErrors: (errors: string[]) => void;
};

type EditorProviderProps<T> = {
  readonly initialValue: T;
  readonly onFinalize: (finalValue: T) => Promise<void> | void;
  readonly onValidate?: (value: T) => string[];
  readonly children: React.ReactNode;
};

// --- Context 생성 ---

// 'any' 대신 'unknown'을 사용하여 linter 오류를 해결하고 타입 안정성을 높입니다.
const EditorContext = createContext<EditorContextValue<unknown> | null>(null);

// --- Provider 컴포넌트 ---

/**
 * 복잡한 폼 상태 관리를 위한 Provider입니다.
 * Immer와 fast-deep-equal 라이브러리를 사용하여 안정성과 성능을 개선했습니다.
 *
 * 필요 라이브러리:
 * npm install immer fast-deep-equal
 */
export function EditorProvider<T extends object>({
  initialValue,
  onFinalize,
  onValidate,
  children,
}: EditorProviderProps<T>) {
  const [draft, setDraft] = useState<T>(initialValue);
  const [isLoading, setIsLoading] = useState(false);
  const [errors, setErrors] = useState<readonly string[]>([]);

  const originalValueRef = useRef<T>(initialValue);

  // initialValue가 외부에서 변경될 경우, 내부 상태를 모두 리셋합니다.
  useEffect(() => {
    originalValueRef.current = initialValue;
    setDraft(initialValue);
    setErrors([]);
  }, [initialValue]);

  // 'fast-deep-equal'을 사용하여 객체/배열의 실제 내용 변경을 안정적으로 감지합니다.
  const isDirty = useMemo(
    () => !equal(draft, originalValueRef.current),
    [draft],
  );

  // 'immer'의 produce 함수를 사용하여 불변성을 유지하며 상태를 안전하고 간편하게 업데이트합니다.
  const update = useCallback(
    (updater: (draft: Draft<T>) => void) => {
      const newValue = produce(draft, updater);
      setDraft(newValue);

      if (onValidate) {
        setErrors(onValidate(newValue));
      }
    },
    [draft, onValidate],
  );

  const commit = useCallback(async () => {
    if (onValidate) {
      const validationErrors = onValidate(draft);
      if (validationErrors.length > 0) {
        setErrors(validationErrors);
        return; // 유효성 검사 실패 시 commit 중단
      }
    }

    setIsLoading(true);
    try {
      await onFinalize(draft);
      // commit 성공 시, 현재 draft를 새로운 원본으로 설정합니다.
      originalValueRef.current = draft;
      setErrors([]);
    } catch (error) {
      console.error('Commit failed:', error);
      const errorMessage =
        error instanceof Error ? error.message : '저장에 실패했습니다.';
      setErrors([errorMessage]);
      throw error;
    } finally {
      setIsLoading(false);
    }
  }, [draft, onFinalize, onValidate]);

  const reset = useCallback(() => {
    setDraft(originalValueRef.current);
    setErrors([]);
  }, []);

  const contextValue = useMemo(
    () => ({
      draft,
      originalValue: originalValueRef.current,
      isLoading,
      isDirty,
      errors,
      update,
      commit,
      reset,
      setErrors,
    }),
    [draft, isLoading, isDirty, errors, update, commit, reset],
  );

  return (
    // Provider에 값을 전달할 때, 구체적인 타입 T를 가진 contextValue를 unknown으로 단언합니다.
    <EditorContext.Provider value={contextValue as EditorContextValue<unknown>}>
      {children}
    </EditorContext.Provider>
  );
}

// --- 훅 (Hooks) ---

/**
 * EditorProvider의 상태와 함수에 접근하기 위한 기본 훅입니다.
 */
export function useEditor<T>(): EditorContextValue<T> {
  const context = useContext(EditorContext);
  if (!context) {
    throw new Error('useEditor must be used within an EditorProvider');
  }
  // Context에서 가져온 'unknown' 타입의 값을 실제 사용될 타입 T로 단언합니다.
  return context as unknown as EditorContextValue<T>;
}

/**
 * 특정 필드의 값과 해당 값을 업데이트하는 함수를 반환하는 헬퍼 훅입니다.
 * @param fieldName - 업데이트할 필드의 키
 */
export function useEditorField<T extends object, K extends keyof T>(
  fieldName: K,
) {
  const { draft, update } = useEditor<T>();

  const value = draft[fieldName];

  const setValue = useCallback(
    (newValue: T[K]) => {
      update((d) => {
        // 'any' 대신 더 안전한 타입 단언을 사용하여 Immer의 Draft 타입을 다룹니다.
        (d as T)[fieldName] = newValue;
      });
    },
    [update, fieldName],
  );

  return [value, setValue] as const;
}
