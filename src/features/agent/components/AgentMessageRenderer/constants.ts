import remarkGfm from 'remark-gfm';
import remarkMath from 'remark-math';
import rehypeKatex from 'rehype-katex';

// Define plugins outside component to maintain stable references
export const REMARK_PLUGINS = [remarkGfm, remarkMath];
export const REHYPE_PLUGINS = [rehypeKatex];

export const SUPPORTED_CONTENT_TYPES = [
  'rawHtml',
  'externalUrl',
  'remoteDom',
] as const;

export const HTML_PROPS = {
  style: { height: 'auto', maxHeight: 'unset' },
  iframeProps: {
    className: 'h-auto min-h-96 max-h-none',
  },
};
