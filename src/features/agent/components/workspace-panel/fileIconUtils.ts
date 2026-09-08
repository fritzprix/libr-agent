import {
  FileText,
  FileCode,
  FileJson,
  FileSpreadsheet,
  Presentation,
  FileImage,
  Archive,
  File,
  type LucideIcon,
} from 'lucide-react';

export type FileCategory =
  | 'document'
  | 'code'
  | 'config'
  | 'spreadsheet'
  | 'presentation'
  | 'image'
  | 'archive'
  | 'pdf'
  | 'default';

export interface FileIconInfo {
  icon: LucideIcon;
  className: string;
  category: FileCategory;
}

const DOCUMENT_EXTENSIONS = new Set([
  'md',
  'markdown',
  'txt',
  'rtf',
  'doc',
  'docx',
  'odt',
]);

const CODE_EXTENSIONS = new Set([
  'ts',
  'tsx',
  'js',
  'jsx',
  'py',
  'rs',
  'go',
  'java',
  'c',
  'cpp',
  'h',
  'css',
  'scss',
  'sh',
  'bash',
  'html',
  'htm',
  'sql',
]);

const CODE_EXACT_NAMES = new Set(['dockerfile', 'makefile']);

const CONFIG_EXTENSIONS = new Set([
  'json',
  'yaml',
  'yml',
  'toml',
  'xml',
  'env',
  'ini',
  'cfg',
  'conf',
]);

const CONFIG_EXACT_NAMES = new Set([
  '.gitignore',
  '.dockerignore',
  '.editorconfig',
  '.env',
]);

const SPREADSHEET_EXTENSIONS = new Set(['csv', 'tsv', 'xlsx', 'xls']);

const PRESENTATION_EXTENSIONS = new Set(['pptx', 'ppt', 'key']);

const IMAGE_EXTENSIONS = new Set([
  'png',
  'jpg',
  'jpeg',
  'gif',
  'svg',
  'webp',
  'ico',
]);

const ARCHIVE_EXTENSIONS = new Set([
  'zip',
  'tar',
  'gz',
  '7z',
  'rar',
  'bz2',
  'xz',
]);

/**
 * Extracts normalized lower-case extension from a filename.
 */
export function getFileExtension(filename: string): string {
  const clean = filename.trim().toLowerCase();
  const parts = clean.split('.');
  if (parts.length <= 1) return clean;
  if (parts.length === 2 && parts[0] === '') return parts[1]; // Hidden file like .gitignore
  return parts[parts.length - 1] ?? '';
}

/**
 * Categorizes a file into one of 8 functional groups or default.
 */
export function getFileCategory(filename: string): FileCategory {
  const lower = filename.trim().toLowerCase();
  const ext = getFileExtension(lower);

  if (ext === 'pdf') return 'pdf';
  if (DOCUMENT_EXTENSIONS.has(ext)) return 'document';
  if (CODE_EXACT_NAMES.has(lower) || CODE_EXTENSIONS.has(ext)) return 'code';
  if (CONFIG_EXACT_NAMES.has(lower) || CONFIG_EXTENSIONS.has(ext))
    return 'config';
  if (SPREADSHEET_EXTENSIONS.has(ext)) return 'spreadsheet';
  if (PRESENTATION_EXTENSIONS.has(ext)) return 'presentation';
  if (IMAGE_EXTENSIONS.has(ext)) return 'image';
  if (ARCHIVE_EXTENSIONS.has(ext)) return 'archive';

  return 'default';
}

/**
 * Returns the Lucide icon and color classes for a file.
 */
export function getFileIconInfo(filename: string): FileIconInfo {
  const category = getFileCategory(filename);

  switch (category) {
    case 'pdf':
      return {
        icon: FileText,
        className: 'text-rose-500 dark:text-rose-400',
        category,
      };
    case 'document':
      return {
        icon: FileText,
        className: 'text-indigo-400 dark:text-indigo-400',
        category,
      };
    case 'code':
      return {
        icon: FileCode,
        className: 'text-sky-400 dark:text-sky-400',
        category,
      };
    case 'config':
      return {
        icon: FileJson,
        className: 'text-amber-400 dark:text-amber-400',
        category,
      };
    case 'spreadsheet':
      return {
        icon: FileSpreadsheet,
        className: 'text-emerald-500 dark:text-emerald-400',
        category,
      };
    case 'presentation':
      return {
        icon: Presentation,
        className: 'text-orange-400 dark:text-orange-400',
        category,
      };
    case 'image':
      return {
        icon: FileImage,
        className: 'text-pink-400 dark:text-pink-400',
        category,
      };
    case 'archive':
      return {
        icon: Archive,
        className: 'text-amber-600 dark:text-amber-500',
        category,
      };
    default:
      return {
        icon: File,
        className: 'text-muted-foreground',
        category,
      };
  }
}

/**
 * Strict allowlist of formats supported by the in-app preview viewer.
 * Excludes PDF, Word (docx), Excel (xlsx), PowerPoint (pptx), Archives, etc.
 */
const PREVIEWABLE_EXTENSIONS = new Set([
  'md',
  'markdown',
  'txt',
  'log',
  'csv',
  'tsv',
  'json',
  'yaml',
  'yml',
  'toml',
  'xml',
  'env',
  'ini',
  'cfg',
  'conf',
  'html',
  'htm',
  'css',
  'scss',
  'js',
  'jsx',
  'ts',
  'tsx',
  'py',
  'rs',
  'go',
  'java',
  'c',
  'cpp',
  'h',
  'sh',
  'bash',
  'sql',
  'png',
  'jpg',
  'jpeg',
  'gif',
  'svg',
  'webp',
  'ico',
]);

const PREVIEWABLE_EXACT_NAMES = new Set([
  'dockerfile',
  'makefile',
  '.gitignore',
  '.dockerignore',
  '.editorconfig',
  '.env',
]);

/**
 * Determines whether a file can be previewed inside the in-app Sheet.
 */
export function isPreviewable(filename: string): boolean {
  const lower = filename.trim().toLowerCase();
  if (PREVIEWABLE_EXACT_NAMES.has(lower)) return true;
  const ext = getFileExtension(lower);
  return PREVIEWABLE_EXTENSIONS.has(ext);
}

/**
 * Maps filename or extension to a Prism/highlight.js compatible language identifier.
 */
export function getLanguageFromFileName(filename: string): string {
  const lower = filename.trim().toLowerCase();
  if (lower === 'dockerfile') return 'docker';
  if (lower === 'makefile') return 'makefile';

  const ext = getFileExtension(lower);
  switch (ext) {
    case 'ts':
    case 'tsx':
      return 'typescript';
    case 'js':
    case 'jsx':
    case 'mjs':
    case 'cjs':
      return 'javascript';
    case 'py':
      return 'python';
    case 'rs':
      return 'rust';
    case 'go':
      return 'go';
    case 'java':
      return 'java';
    case 'c':
    case 'h':
      return 'c';
    case 'cpp':
      return 'cpp';
    case 'html':
    case 'htm':
      return 'html';
    case 'css':
    case 'scss':
      return 'css';
    case 'json':
      return 'json';
    case 'yaml':
    case 'yml':
      return 'yaml';
    case 'toml':
      return 'toml';
    case 'xml':
      return 'xml';
    case 'ini':
    case 'cfg':
    case 'conf':
    case 'env':
      return 'ini';
    case 'md':
    case 'markdown':
      return 'markdown';
    case 'sh':
    case 'bash':
      return 'bash';
    case 'sql':
      return 'sql';
    default:
      return 'text';
  }
}
