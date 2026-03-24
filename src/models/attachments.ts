export interface AttachmentItem {
  sessionId: string;
  contentId: string;
  filename: string;
  mimeType: string;
  size: number;
  lineCount: number;
  preview: string;
  uploadedAt: string;
  chunkCount: number;
  lastAccessedAt?: string;
}

export interface ListAttachmentsResult {
  sessionId: string;
  contents: AttachmentItem[];
  total: number;
  hasMore: boolean;
}

// Backend DTOs
export interface AddAttachmentMetadata {
  filename?: string;
  mimeType?: string;
  size?: number;
  uploadedAt?: string;
}

export interface AddAttachmentArgs {
  fileUrl?: string; // Local file path or blob URL
  srcUrl?: string; // Source URL if web content
  content?: string; // Direct text content
  metadata?: AddAttachmentMetadata;
  title?: string;
  tags?: string[];
}

export interface CreateAttachmentStoreArgs {
  sessionId?: string;
  metadata?: Record<string, unknown>;
}

export interface CreateAttachmentStoreResult {
  sessionId: string;
}

export interface ListAttachmentsArgs {
  sessionId?: string;
  pagination?: {
    limit?: number;
    offset?: number;
  };
}

export interface DeleteAttachmentArgs {
  contentId: string;
}
