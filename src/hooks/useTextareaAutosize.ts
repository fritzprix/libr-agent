import { useLayoutEffect, type RefObject } from 'react';

interface UseTextareaAutosizeOptions {
  textareaRef: RefObject<HTMLTextAreaElement>;
  value: string;
  maxHeight: number;
}

export function useTextareaAutosize({
  textareaRef,
  value,
  maxHeight,
}: UseTextareaAutosizeOptions) {
  useLayoutEffect(() => {
    const textarea = textareaRef.current;
    if (!textarea) return;

    textarea.style.height = 'auto';
    const nextHeight = Math.min(textarea.scrollHeight, maxHeight);
    textarea.style.height = `${nextHeight}px`;
  }, [maxHeight, textareaRef, value]);
}
