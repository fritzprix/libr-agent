import { getLogger } from '@/lib/logger';

const logger = getLogger('clipboard-image-utils');

export function extractImagesFromHTML(htmlText: string): File[] {
  const parser = new DOMParser();
  const doc = parser.parseFromString(htmlText, 'text/html');
  const imgs = doc.querySelectorAll('img');
  const files: File[] = [];

  imgs.forEach((img) => {
    const src = img.getAttribute('src');
    if (src && src.startsWith('data:image/')) {
      try {
        const parts = src.split(',');
        if (parts.length < 2 || !parts[1]) return;
        const mimeMatch = parts[0].match(/data:(image\/[^;]+)/);
        const isBase64 = parts[0].includes('base64');
        if (!mimeMatch) return;
        const mime = mimeMatch[1];

        let u8arr: Uint8Array;
        if (isBase64) {
          const bstr = atob(parts[1]);
          let n = bstr.length;
          u8arr = new Uint8Array(n);
          while (n--) {
            u8arr[n] = bstr.charCodeAt(n);
          }
        } else {
          const decoded = decodeURIComponent(parts[1]);
          let n = decoded.length;
          u8arr = new Uint8Array(n);
          while (n--) {
            u8arr[n] = decoded.charCodeAt(n);
          }
        }

        const file = new File([u8arr], '', { type: mime });
        files.push(file);
      } catch (err) {
        logger.error('Failed to parse image from HTML:', err);
      }
    }
  });

  return files;
}
