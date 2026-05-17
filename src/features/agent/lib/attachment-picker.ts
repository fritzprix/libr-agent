const AGENT_ATTACHMENT_PICKER_EXTENSION_ACCEPT = [
  '.txt',
  '.md',
  '.json',
  '.js',
  '.ts',
  '.tsx',
  '.jsx',
  '.py',
  '.java',
  '.cpp',
  '.c',
  '.h',
  '.css',
  '.html',
  '.xml',
  '.yaml',
  '.yml',
  '.csv',
  '.pdf',
  '.docx',
  '.xlsx',
];

export const AGENT_ATTACHMENT_PICKER_ACCEPT = [
  ...AGENT_ATTACHMENT_PICKER_EXTENSION_ACCEPT,
  'image/*',
  'audio/*',
].join(',');
