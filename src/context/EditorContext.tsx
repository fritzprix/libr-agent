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
import { getLogger } from '@/lib/logger';

const logger = getLogger('EditorContext');

// --- Type Definitions ---

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

// --- Context Creation ---

// Use 'unknown' instead of 'any' to satisfy the linter and improve type safety.
const EditorContext = createContext<EditorContextValue<unknown> | null>(null);

// --- Provider Component ---

/**
 * Provider for managing complex form state.
 * Uses Immer and fast-deep-equal to improve stability and performance.
 *
 * Required libraries:
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
  const [prevInitialValue, setPrevInitialValue] = useState<T>(initialValue);

  // Render-phase mutation to sync state from props safely without extra render cycle
  if (initialValue !== prevInitialValue) {
    setPrevInitialValue(initialValue);
    originalValueRef.current = initialValue;
    setDraft(initialValue);
    setErrors([]);
  }

  // Use 'fast-deep-equal' to reliably detect actual content changes in objects/arrays.
  const isDirty = useMemo(
    () => !equal(draft, originalValueRef.current),
    [draft],
  );

  // Use immer's produce to update state safely and immutably.
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
        return; // Abort commit if validation fails
      }
    }

    setIsLoading(true);
    try {
      await onFinalize(draft);
      // On successful commit, set the current draft as the new original.
      originalValueRef.current = draft;
      setErrors([]);
    } catch (error) {
      logger.error('Commit failed:', error);
      const errorMessage =
        error instanceof Error ? error.message : 'Failed to save.';
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
    // Assert contextValue (typed T) as unknown when passing to Provider to satisfy TypeScript.
    <EditorContext.Provider value={contextValue as EditorContextValue<unknown>}>
      {children}
    </EditorContext.Provider>
  );
}

// --- Hooks ---

/**
 * Base hook for accessing EditorProvider state and functions.
 */
export function useEditor<T>(): EditorContextValue<T> {
  const context = useContext(EditorContext);
  if (!context) {
    throw new Error('useEditor must be used within an EditorProvider');
  }
  // Assert the 'unknown' context value to the concrete type T expected by the caller.
  return context as unknown as EditorContextValue<T>;
}

/**
 * A helper hook that returns the value of a specific field and a function to update that value.
 * @param fieldName The key of the field to update
 */
export function useEditorField<T extends object, K extends keyof T>(
  fieldName: K,
) {
  const { draft, update } = useEditor<T>();

  const value = draft[fieldName];

  const setValue = useCallback(
    (newValue: T[K]) => {
      update((d) => {
        // Use a safer type assertion instead of 'any' to handle Immer's Draft type.
        (d as T)[fieldName] = newValue;
      });
    },
    [update, fieldName],
  );

  return [value, setValue] as const;
}
