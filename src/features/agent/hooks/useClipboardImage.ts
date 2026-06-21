import { useCallback } from 'react';
import { getLogger } from '@/lib/logger';
import { extractImagesFromHTML } from '../lib/clipboard-image-utils';

const logger = getLogger('useClipboardImage');

interface UseClipboardImageProps {
  input: string;
  setInput: (val: string) => void;
  onInputChange: (val: string, cursorPos: number) => void;
  textareaRef: React.RefObject<HTMLTextAreaElement>;
  attachFiles: (files: File[]) => void | Promise<void>;
}

export function useClipboardImage({
  input,
  setInput,
  onInputChange,
  textareaRef,
  attachFiles,
}: UseClipboardImageProps) {
  const insertTextAtSelection = useCallback(
    (text: string) => {
      const textarea = textareaRef.current;
      const selectionStart = textarea?.selectionStart ?? input.length;
      const selectionEnd = textarea?.selectionEnd ?? input.length;
      const nextValue =
        input.slice(0, selectionStart) + text + input.slice(selectionEnd);
      const nextCursorPosition = selectionStart + text.length;

      setInput(nextValue);
      onInputChange(nextValue, nextCursorPosition);

      requestAnimationFrame(() => {
        textareaRef.current?.focus();
        textareaRef.current?.setSelectionRange(
          nextCursorPosition,
          nextCursorPosition,
        );
      });
    },
    [input, onInputChange, setInput, textareaRef],
  );

  const handlePaste = useCallback(
    (e: React.ClipboardEvent<HTMLTextAreaElement>) => {
      const clipboardData = e.clipboardData;
      if (!clipboardData) return;

      const items = clipboardData.items ? Array.from(clipboardData.items) : [];
      const files = clipboardData.files ? Array.from(clipboardData.files) : [];

      const imageFilesFromItems = items
        .filter(
          (item) => item.kind === 'file' && item.type.startsWith('image/'),
        )
        .flatMap((item) => {
          const file = item.getAsFile();
          return file ? [file] : [];
        });
      let imageFiles =
        imageFilesFromItems.length > 0
          ? imageFilesFromItems
          : files.filter((file) => file.type.startsWith('image/'));

      if (imageFiles.length === 0) {
        const htmlText = clipboardData.getData('text/html');
        if (htmlText) {
          imageFiles = extractImagesFromHTML(htmlText);
        }
      }

      if (imageFiles.length === 0) {
        // Fallback: Try reading from Async Clipboard API if types/items/files were empty
        if (
          navigator.clipboard &&
          typeof navigator.clipboard.read === 'function'
        ) {
          navigator.clipboard
            .read()
            .then(async (clipboardItems) => {
              const extractedFiles: File[] = [];
              for (const item of clipboardItems) {
                const imageTypes = item.types.filter((t) =>
                  t.startsWith('image/'),
                );
                for (const type of imageTypes) {
                  try {
                    const blob = await item.getType(type);
                    const file = new File([blob], '', { type });
                    extractedFiles.push(file);
                  } catch (err) {
                    logger.error(
                      'Failed to get blob from clipboard item:',
                      err,
                    );
                  }
                }
              }
              if (extractedFiles.length > 0) {
                void attachFiles(extractedFiles);
              }
            })
            .catch((err) => {
              logger.warn('Failed to read from Async Clipboard API:', err);
            });
        }
        return;
      }

      e.preventDefault();

      const pastedText = clipboardData.getData('text/plain');
      if (pastedText) {
        insertTextAtSelection(pastedText);
      }

      void attachFiles(imageFiles);
    },
    [attachFiles, insertTextAtSelection],
  );

  return { handlePaste };
}
