import { useState } from 'react';
import { Check, Copy, Download } from 'lucide-react';
import { toast } from 'sonner';
import { useRustBackend } from '@/hooks/use-rust-backend';
import { Tooltip, TooltipTrigger, TooltipContent } from '@/components/ui';
import { getLogger } from '@/lib/logger';
import { useResolvedMediaSource } from '../hooks/useResolvedMediaSource';

const logger = getLogger('AgentMessageRenderer');

export interface MediaRendererProps {
  rawData?: string;
  uri?: string;
  mimeType: string;
  itemKey: string;
  sessionId?: string;
}

function decodeBase64ToBytes(value: string): Uint8Array {
  const normalized = value.replace(/\s+/g, '');
  const binary = atob(normalized);
  const bytes = new Uint8Array(binary.length);

  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }

  return bytes;
}

function dataUrlToBlob(dataUrl: string, fallbackMimeType: string): Blob {
  const separatorIndex = dataUrl.indexOf(',');
  if (separatorIndex === -1) {
    throw new Error('Invalid data URL');
  }

  const metadata = dataUrl.slice(0, separatorIndex);
  const payload = dataUrl.slice(separatorIndex + 1);
  const mimeTypeMatch = metadata.match(/^data:([^;,]+)/);
  const resolvedMimeType = mimeTypeMatch?.[1] || fallbackMimeType;

  if (metadata.includes(';base64')) {
    return new Blob([decodeBase64ToBytes(payload)], {
      type: resolvedMimeType,
    });
  }

  return new Blob([decodeURIComponent(payload)], {
    type: resolvedMimeType,
  });
}

async function resolveImageBlob(
  rawData: string | undefined,
  imageSrc: string,
  mimeType: string,
): Promise<Blob> {
  if (rawData) {
    return rawData.startsWith('data:')
      ? dataUrlToBlob(rawData, mimeType)
      : new Blob([decodeBase64ToBytes(rawData)], { type: mimeType });
  }

  if (imageSrc.startsWith('data:')) {
    return dataUrlToBlob(imageSrc, mimeType);
  }

  const response = await fetch(imageSrc);
  if (!response.ok) {
    throw new Error(`Failed to read image source: ${response.status}`);
  }

  const blob = await response.blob();
  if (blob.type) {
    return blob;
  }

  return new Blob([await blob.arrayBuffer()], { type: mimeType });
}

function getImageDownloadName(
  uri: string | undefined,
  mimeType: string,
): string {
  const uriSegment = uri?.split(/[?#]/u, 1)[0]?.split('/').pop();
  if (uriSegment && uriSegment.includes('.')) {
    return uriSegment;
  }

  const extension = mimeType.split('/')[1]?.split('+')[0] || 'png';
  return `image-${Date.now()}.${extension}`;
}

function canWriteImagesToClipboard(): boolean {
  return (
    typeof navigator !== 'undefined' &&
    typeof navigator.clipboard?.write === 'function' &&
    typeof ClipboardItem !== 'undefined'
  );
}

function MediaLoadError({ label, detail }: { label: string; detail?: string }) {
  return (
    <div
      role="status"
      className="rounded-lg border border-dashed border-muted-foreground/30 bg-muted/40 px-3 py-2 text-sm text-muted-foreground"
    >
      <div className="font-medium text-foreground/80">{`Failed to load ${label}`}</div>
      {detail ? <div className="mt-1 text-xs">{detail}</div> : null}
    </div>
  );
}

export function ImageContentRenderer({
  rawData,
  uri,
  mimeType,
  itemKey,
  sessionId,
}: MediaRendererProps) {
  const { downloadMediaFile } = useRustBackend();
  const { resolvedSrc: imageSrc, loadError } = useResolvedMediaSource(
    rawData,
    uri,
    mimeType,
    sessionId,
  );
  const [copied, setCopied] = useState(false);
  const canCopyImage = canWriteImagesToClipboard();
  const copyButtonClassName =
    'flex items-center justify-center rounded p-1.5 text-muted-foreground transition-colors hover:bg-secondary hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50';

  if (!imageSrc) {
    return loadError ? (
      <MediaLoadError label="image" detail={loadError} />
    ) : null;
  }

  const handleCopy = async () => {
    if (!canCopyImage) {
      toast.error('Image clipboard is not supported in this environment');
      return;
    }

    try {
      const blob = await resolveImageBlob(rawData, imageSrc, mimeType);
      const clipboardMimeType = blob.type || mimeType;
      await navigator.clipboard.write([
        new ClipboardItem({
          [clipboardMimeType]: blob,
        }),
      ]);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
      toast.success('Image copied to clipboard');
    } catch (error) {
      logger.error('Failed to copy image to clipboard', error);
      toast.error('Failed to copy image to clipboard');
    }
  };

  const handleDownload = async () => {
    const fileName = getImageDownloadName(uri, mimeType);
    const dataBase64 =
      rawData && !rawData.startsWith('data:')
        ? rawData
        : imageSrc.startsWith('data:')
          ? imageSrc.slice(imageSrc.indexOf(',') + 1)
          : undefined;
    const fileUrl =
      dataBase64 === undefined && !rawData && uri?.startsWith('file://')
        ? uri
        : undefined;

    try {
      const result = await downloadMediaFile({
        sessionId,
        fileName,
        mimeType,
        dataBase64,
        fileUrl,
      });

      if (result === 'Download cancelled by user') {
        toast.info('Download cancelled');
        return;
      }

      toast.success(result);
    } catch (error) {
      logger.error('Failed to download image', error);
      toast.error('Failed to download image');
    }
  };

  const copyButton = (
    <button
      type="button"
      onClick={handleCopy}
      disabled={!canCopyImage}
      className={copyButtonClassName}
      aria-label="Copy image to clipboard"
    >
      {copied ? (
        <Check size={16} className="text-emerald-500" />
      ) : (
        <Copy size={16} />
      )}
    </button>
  );

  return (
    <div className="group relative inline-block max-w-full">
      <div className="absolute top-2 right-2 z-10 flex items-center gap-1 rounded-md border border-border bg-background/80 p-1 opacity-0 shadow-sm backdrop-blur-sm transition-opacity group-hover:opacity-100 focus-within:opacity-100">
        <Tooltip>
          {canCopyImage ? (
            <TooltipTrigger asChild>{copyButton}</TooltipTrigger>
          ) : (
            <TooltipTrigger asChild>
              <span
                tabIndex={0}
                role="button"
                aria-label="Copy image to clipboard"
                aria-disabled="true"
                className="flex items-center justify-center rounded focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              >
                {copyButton}
              </span>
            </TooltipTrigger>
          )}
          <TooltipContent>
            {canCopyImage
              ? 'Copy Image'
              : 'Image clipboard is not supported in this environment'}
          </TooltipContent>
        </Tooltip>

        <Tooltip>
          <TooltipTrigger asChild>
            <button
              type="button"
              onClick={handleDownload}
              className="flex items-center justify-center rounded p-1.5 text-muted-foreground transition-colors hover:bg-secondary hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              aria-label="Download image"
            >
              <Download size={16} />
            </button>
          </TooltipTrigger>
          <TooltipContent>Download Image</TooltipContent>
        </Tooltip>
      </div>

      <img
        key={itemKey}
        src={imageSrc}
        alt="Tool output"
        className="h-auto max-w-full rounded-lg border border-border/10 shadow-sm"
      />
    </div>
  );
}

export function AudioContentRenderer({
  rawData,
  uri,
  mimeType,
  itemKey,
  sessionId,
}: MediaRendererProps) {
  const { resolvedSrc: audioSrc, loadError } = useResolvedMediaSource(
    rawData,
    uri,
    mimeType,
    sessionId,
  );

  if (!audioSrc) {
    return loadError ? (
      <MediaLoadError label="audio" detail={loadError} />
    ) : null;
  }

  return (
    <audio key={itemKey} controls className="w-full">
      <source src={audioSrc} type={mimeType} />
      Your browser does not support the audio element.
    </audio>
  );
}

export function VideoContentRenderer({
  rawData,
  uri,
  mimeType,
  itemKey,
  sessionId,
}: MediaRendererProps) {
  const { resolvedSrc: videoSrc, loadError } = useResolvedMediaSource(
    rawData,
    uri,
    mimeType,
    sessionId,
  );

  if (!videoSrc) {
    return loadError ? (
      <MediaLoadError label="video" detail={loadError} />
    ) : null;
  }

  return (
    <video key={itemKey} controls className="w-full rounded-lg shadow-sm">
      <source src={videoSrc} type={mimeType} />
      Your browser does not support the video element.
    </video>
  );
}
