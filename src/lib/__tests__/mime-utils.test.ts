import { describe, it, expect } from 'vitest';
import { getMimeTypeFromFilename } from '../mime-utils';

describe('getMimeTypeFromFilename', () => {
  describe('image types', () => {
    it.each([
      ['photo.jpg', 'image/jpeg'],
      ['photo.jpeg', 'image/jpeg'],
      ['image.png', 'image/png'],
      ['anim.gif', 'image/gif'],
      ['photo.webp', 'image/webp'],
      ['icon.svg', 'image/svg+xml'],
      ['bitmap.bmp', 'image/bmp'],
      ['favicon.ico', 'image/x-icon'],
      ['scan.tiff', 'image/tiff'],
      ['scan.tif', 'image/tiff'],
    ])('%s → %s', (filename, expected) => {
      expect(getMimeTypeFromFilename(filename)).toBe(expected);
    });
  });

  describe('audio types', () => {
    it.each([
      ['track.mp3', 'audio/mpeg'],
      ['recording.wav', 'audio/wav'],
      ['clip.ogg', 'audio/ogg'],
      ['lossless.flac', 'audio/flac'],
      ['voice.m4a', 'audio/mp4'],
      ['compressed.aac', 'audio/aac'],
      ['video.webm', 'audio/webm'],
    ])('%s → %s', (filename, expected) => {
      expect(getMimeTypeFromFilename(filename)).toBe(expected);
    });
  });

  describe('text / document types', () => {
    it.each([
      ['readme.txt', 'text/plain'],
      ['README.md', 'text/markdown'],
      ['data.json', 'application/json'],
      ['report.pdf', 'application/pdf'],
      ['document.docx', 'application/vnd.openxmlformats-officedocument.wordprocessingml.document'],
      ['spreadsheet.xlsx', 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet'],
    ])('%s → %s', (filename, expected) => {
      expect(getMimeTypeFromFilename(filename)).toBe(expected);
    });
  });

  describe('unknown / fallback', () => {
    it('returns application/octet-stream for unknown extension', () => {
      expect(getMimeTypeFromFilename('binary.bin')).toBe('application/octet-stream');
    });

    it('returns application/octet-stream for no extension', () => {
      expect(getMimeTypeFromFilename('Makefile')).toBe('application/octet-stream');
    });

    it('is case-insensitive for extensions', () => {
      expect(getMimeTypeFromFilename('Photo.JPG')).toBe('image/jpeg');
      expect(getMimeTypeFromFilename('AUDIO.MP3')).toBe('audio/mpeg');
      expect(getMimeTypeFromFilename('Image.PNG')).toBe('image/png');
    });

    it('handles files with multiple dots correctly', () => {
      expect(getMimeTypeFromFilename('my.photo.archive.jpg')).toBe('image/jpeg');
    });
  });

  describe('regression: previously missing image/audio types (Linux DnD bug)', () => {
    // These are the types that were missing from the old local getMimeType implementations,
    // causing Linux/WebKitGTK drag-and-drop to produce application/octet-stream for images.
    it('jpeg returns image/jpeg (not octet-stream)', () => {
      const result = getMimeTypeFromFilename('photo.jpeg');
      expect(result).toBe('image/jpeg');
      expect(result).not.toBe('application/octet-stream');
    });

    it('png returns image/png (not octet-stream)', () => {
      const result = getMimeTypeFromFilename('screenshot.png');
      expect(result).toBe('image/png');
      expect(result).not.toBe('application/octet-stream');
    });

    it('wav returns audio/wav (not octet-stream)', () => {
      const result = getMimeTypeFromFilename('audio.wav');
      expect(result).toBe('audio/wav');
      expect(result).not.toBe('application/octet-stream');
    });
  });
});
