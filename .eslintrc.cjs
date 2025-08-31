// .eslintrc.cjs
module.exports = {
  root: true,
  env: { browser: true, es2020: true, node: true },
  extends: [
    'eslint:recommended',
    'plugin:@typescript-eslint/recommended',
    'plugin:react-hooks/recommended',
    'prettier', // 반드시 가장 마지막에 추가!
  ],
  ignorePatterns: [
    'dist',
    '.eslintrc.cjs',
    'src-tauri/',
    'src-tauri/target/',
    'node_modules/',
    'public/',
  ], // 빌드/자동생성 파일 린트 제외
  parser: '@typescript-eslint/parser',
  plugins: ['react-refresh'],
  rules: {
    'react-refresh/only-export-components': [
      'warn',
      { allowConstantExport: true },
    ],
    '@typescript-eslint/no-unused-vars': 'warn', // 사용하지 않는 변수는 경고 표시
  },
};
