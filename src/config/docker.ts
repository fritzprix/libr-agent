/**
 * Docker image presets displayed in the workspace isolation settings.
 * Add new entries here to make them available in the draft chat UI.
 * An empty `val` selects the LibrAgent default image (resolved by the backend).
 */
export const DOCKER_IMAGE_PRESETS = [
  { label: 'LibrAgent', val: '' },
  { label: 'Python 3', val: 'python:3.11-slim' },
  { label: 'Node 20', val: 'node:20-alpine' },
  { label: 'Ubuntu', val: 'ubuntu:latest' },
  { label: 'Go 1.22', val: 'golang:1.22-alpine' },
] as const;

export type DockerImagePreset = (typeof DOCKER_IMAGE_PRESETS)[number];
