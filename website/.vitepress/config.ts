import { createRequire } from 'node:module';
import { dirname, join } from 'node:path';
import { defineConfig } from 'vitepress';

const require = createRequire(import.meta.url);
const vueRoot = dirname(require.resolve('vue/package.json'));
const repo = 'https://github.com/fritzprix/libr-agent';

export default defineConfig({
  title: 'LibrAgent',
  description: 'LibrAgent user documentation — install, chat, and connect models',
  lang: 'ko-KR',
  // Project Pages: https://fritzprix.github.io/libr-agent/
  base: '/libr-agent/',
  srcDir: '../docs/user',
  // GitHub folder view uses README.md; the site home is index.md.
  srcExclude: [
    '**/README.md',
    '**/guides/navigation-guide.md',
  ],
  cleanUrls: true,
  lastUpdated: true,
  // Relative GitHub-style links (.md) and WIP pages are OK while docs grow.
  ignoreDeadLinks: true,
  head: [['meta', { name: 'theme-color', content: '#0f766e' }]],
  vite: {
    resolve: {
      alias: {
        vue: vueRoot,
        'vue/server-renderer': join(vueRoot, 'server-renderer'),
      },
      dedupe: ['vue'],
    },
    server: {
      fs: {
        allow: ['..'],
      },
    },
  },
  themeConfig: {
    nav: [
      { text: '시작하기', link: '/getting-started/5-minute-tutorial' },
      { text: '모델 연결', link: '/getting-started/connecting-models' },
      { text: '문제 해결', link: '/guides/troubleshooting' },
      { text: 'GitHub', link: repo },
      { text: 'Releases', link: `${repo}/releases/latest` },
    ],
    sidebar: [
      {
        text: '시작하기',
        items: [
          { text: '소개', link: '/' },
          {
            text: '5분 시작 가이드',
            link: '/getting-started/5-minute-tutorial',
          },
          {
            text: '에이전트 첫 대화',
            link: '/getting-started/first-agent',
          },
          {
            text: '모델 연결하기',
            link: '/getting-started/connecting-models',
          },
        ],
      },
      {
        text: '가이드',
        items: [
          {
            text: '스킬 (scope·배포)',
            link: '/guides/skills',
          },
          {
            text: '서브 에이전트 · 오케스트레이션',
            link: '/guides/sub-agents',
          },
          {
            text: 'Assistants',
            link: '/guides/assistants',
          },
          {
            text: 'Playbooks',
            link: '/guides/playbooks',
          },
          {
            text: '자동화 (Scheduled Tasks)',
            link: '/guides/automation',
          },
          {
            text: 'Extensions (MCP)',
            link: '/guides/extensions',
          },
          {
            text: '커스텀 MCP',
            link: '/guides/custom-mcp',
          },
          {
            text: '세션',
            link: '/guides/sessions',
          },
          {
            text: '문제 해결',
            link: '/guides/troubleshooting',
          },
        ],
      },
      {
        text: 'FAQ',
        items: [
          {
            text: '자주 묻는 질문',
            link: '/faq/common-questions',
          },
          {
            text: '에러 코드',
            link: '/faq/error-codes',
          },
        ],
      },
      {
        text: '시나리오',
        items: [
          {
            text: '코드 리뷰',
            link: '/scenarios/code-review',
          },
          {
            text: '조사·리포트',
            link: '/scenarios/research',
          },
          {
            text: '파일 처리',
            link: '/scenarios/file-management',
          },
          {
            text: '웹 브라우징',
            link: '/scenarios/web-browsing',
          },
        ],
      },
    ],
    socialLinks: [{ icon: 'github', link: repo }],
    editLink: {
      pattern: `${repo}/edit/main/docs/user/:path`,
      text: 'GitHub에서 이 페이지 수정',
    },
    footer: {
      message: 'Released under the MIT License.',
      copyright: 'Copyright © LibrAgent contributors',
    },
    search: {
      provider: 'local',
    },
  },
});
