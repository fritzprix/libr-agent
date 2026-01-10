import { AttachmentReference } from '../chat';

// Legacy types moved from content-store.ts for ResourceAttachmentContext usage
export interface PendingFileInput {
  file: File;
  previewUrl?: string;
  url?: string;
  mimeType?: string;
  filename?: string;
  blobCleanup?: () => void;
  originalPath?: string;
}

export interface ExtendedAttachmentReference extends AttachmentReference {
  error?: string;
  isLoading?: boolean;
}
