import { describe, it, expect } from 'vitest';
import {
  getFileExtension,
  getFileCategory,
  getFileIconInfo,
  isPreviewable,
  getLanguageFromFileName,
} from '../fileIconUtils';
import {
  FileText,
  FileCode,
  FileJson,
  FileSpreadsheet,
  Presentation,
  FileImage,
  Archive,
  File,
} from 'lucide-react';

describe('fileIconUtils', () => {
  describe('getFileExtension', () => {
    it('extracts basic extension', () => {
      expect(getFileExtension('readme.md')).toBe('md');
      expect(getFileExtension('App.tsx')).toBe('tsx');
      expect(getFileExtension('image.PNG')).toBe('png');
    });

    it('handles hidden files and dotless files', () => {
      expect(getFileExtension('.gitignore')).toBe('gitignore');
      expect(getFileExtension('.env')).toBe('env');
      expect(getFileExtension('Dockerfile')).toBe('dockerfile');
      expect(getFileExtension('Makefile')).toBe('makefile');
    });

    it('handles compound dots', () => {
      expect(getFileExtension('archive.tar.gz')).toBe('gz');
      expect(getFileExtension('button.test.tsx')).toBe('tsx');
    });
  });

  describe('getFileCategory and getFileIconInfo', () => {
    it('categorizes documents including Word docs', () => {
      expect(getFileCategory('report.docx')).toBe('document');
      expect(getFileCategory('doc.doc')).toBe('document');
      expect(getFileCategory('notes.txt')).toBe('document');
      expect(getFileCategory('README.md')).toBe('document');

      const info = getFileIconInfo('report.docx');
      expect(info.icon).toBe(FileText);
      expect(info.category).toBe('document');
      expect(info.className).toContain('text-indigo-400');
    });

    it('categorizes code and exact scripts', () => {
      expect(getFileCategory('main.rs')).toBe('code');
      expect(getFileCategory('script.py')).toBe('code');
      expect(getFileCategory('index.html')).toBe('code');
      expect(getFileCategory('Dockerfile')).toBe('code');
      expect(getFileCategory('Makefile')).toBe('code');

      const info = getFileIconInfo('main.rs');
      expect(info.icon).toBe(FileCode);
      expect(info.category).toBe('code');
      expect(info.className).toContain('text-sky-400');
    });

    it('categorizes configs and data', () => {
      expect(getFileCategory('package.json')).toBe('config');
      expect(getFileCategory('config.yaml')).toBe('config');
      expect(getFileCategory('.gitignore')).toBe('config');

      const info = getFileIconInfo('package.json');
      expect(info.icon).toBe(FileJson);
      expect(info.category).toBe('config');
      expect(info.className).toContain('text-amber-400');
    });

    it('categorizes spreadsheets', () => {
      expect(getFileCategory('data.csv')).toBe('spreadsheet');
      expect(getFileCategory('table.xlsx')).toBe('spreadsheet');

      const info = getFileIconInfo('table.xlsx');
      expect(info.icon).toBe(FileSpreadsheet);
      expect(info.category).toBe('spreadsheet');
      expect(info.className).toContain('text-emerald-500');
    });

    it('categorizes presentations', () => {
      expect(getFileCategory('slides.pptx')).toBe('presentation');

      const info = getFileIconInfo('slides.pptx');
      expect(info.icon).toBe(Presentation);
      expect(info.category).toBe('presentation');
      expect(info.className).toContain('text-orange-400');
    });

    it('categorizes images', () => {
      expect(getFileCategory('logo.png')).toBe('image');
      expect(getFileCategory('vector.svg')).toBe('image');

      const info = getFileIconInfo('logo.png');
      expect(info.icon).toBe(FileImage);
      expect(info.category).toBe('image');
      expect(info.className).toContain('text-pink-400');
    });

    it('categorizes archives', () => {
      expect(getFileCategory('backup.zip')).toBe('archive');
      expect(getFileCategory('data.tar.gz')).toBe('archive');

      const info = getFileIconInfo('backup.zip');
      expect(info.icon).toBe(Archive);
      expect(info.category).toBe('archive');
      expect(info.className).toContain('text-amber-600');
    });

    it('categorizes pdf separately', () => {
      expect(getFileCategory('manual.pdf')).toBe('pdf');

      const info = getFileIconInfo('manual.pdf');
      expect(info.icon).toBe(FileText);
      expect(info.category).toBe('pdf');
      expect(info.className).toContain('text-rose-500');
    });

    it('falls back to default for unmapped formats', () => {
      expect(getFileCategory('binary.bin')).toBe('default');

      const info = getFileIconInfo('binary.bin');
      expect(info.icon).toBe(File);
      expect(info.category).toBe('default');
    });
  });

  describe('isPreviewable', () => {
    it('allows markdown, code, config, text, html, images, and csv/tsv', () => {
      expect(isPreviewable('README.md')).toBe(true);
      expect(isPreviewable('index.html')).toBe(true);
      expect(isPreviewable('app.tsx')).toBe(true);
      expect(isPreviewable('main.py')).toBe(true);
      expect(isPreviewable('data.json')).toBe(true);
      expect(isPreviewable('notes.txt')).toBe(true);
      expect(isPreviewable('debug.log')).toBe(true);
      expect(isPreviewable('table.csv')).toBe(true);
      expect(isPreviewable('matrix.tsv')).toBe(true);
      expect(isPreviewable('banner.png')).toBe(true);
      expect(isPreviewable('diagram.svg')).toBe(true);
      expect(isPreviewable('Dockerfile')).toBe(true);
      expect(isPreviewable('Makefile')).toBe(true);
      expect(isPreviewable('.gitignore')).toBe(true);
      expect(isPreviewable('.dockerignore')).toBe(true);
      expect(isPreviewable('.env')).toBe(true);
      expect(isPreviewable('settings.ini')).toBe(true);
      expect(isPreviewable('config.yaml')).toBe(true);
      expect(isPreviewable('app.cfg')).toBe(true);
      expect(isPreviewable('server.conf')).toBe(true);
    });

    it('strictly rejects non-previewable formats: docx, xlsx, pptx, pdf, zip', () => {
      expect(isPreviewable('doc.docx')).toBe(false);
      expect(isPreviewable('doc.doc')).toBe(false);
      expect(isPreviewable('sheet.xlsx')).toBe(false);
      expect(isPreviewable('sheet.xls')).toBe(false);
      expect(isPreviewable('slides.pptx')).toBe(false);
      expect(isPreviewable('manual.pdf')).toBe(false);
      expect(isPreviewable('bundle.zip')).toBe(false);
      expect(isPreviewable('binary.exe')).toBe(false);
    });
  });

  describe('getLanguageFromFileName', () => {
    it('returns appropriate language syntax tags', () => {
      expect(getLanguageFromFileName('app.tsx')).toBe('typescript');
      expect(getLanguageFromFileName('index.js')).toBe('javascript');
      expect(getLanguageFromFileName('main.rs')).toBe('rust');
      expect(getLanguageFromFileName('server.py')).toBe('python');
      expect(getLanguageFromFileName('index.html')).toBe('html');
      expect(getLanguageFromFileName('style.css')).toBe('css');
      expect(getLanguageFromFileName('data.json')).toBe('json');
      expect(getLanguageFromFileName('Dockerfile')).toBe('docker');
      expect(getLanguageFromFileName('settings.ini')).toBe('ini');
      expect(getLanguageFromFileName('.env')).toBe('ini');
      expect(getLanguageFromFileName('app.cfg')).toBe('ini');
    });
  });
});
