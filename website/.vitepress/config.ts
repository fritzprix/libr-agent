import { createRequire } from 'node:module';
import { dirname, join } from 'node:path';
import { defineConfig } from 'vitepress';

const require = createRequire(import.meta.url);
const vueRoot = dirname(require.resolve('vue/package.json'));
const repo = 'https://github.com/fritzprix/libr-agent';

const koNav = [
  { text: '시작하기', link: '/getting-started/5-minute-tutorial' },
  { text: '모델 연결', link: '/getting-started/connecting-models' },
  { text: '문제 해결', link: '/guides/troubleshooting' },
  { text: 'GitHub', link: repo },
  { text: 'Releases', link: `${repo}/releases/latest` },
];

const enNav = [
  { text: 'Get started', link: '/en/getting-started/5-minute-tutorial' },
  { text: 'Connect models', link: '/en/getting-started/connecting-models' },
  { text: 'Troubleshooting', link: '/en/guides/troubleshooting' },
  { text: 'GitHub', link: repo },
  { text: 'Releases', link: `${repo}/releases/latest` },
];

const koSidebar = [
  {
    text: '시작하기',
    items: [
      { text: '소개', link: '/' },
      { text: '5분 시작 가이드', link: '/getting-started/5-minute-tutorial' },
      { text: '에이전트 첫 대화', link: '/getting-started/first-agent' },
      { text: '모델 연결하기', link: '/getting-started/connecting-models' },
    ],
  },
  {
    text: '가이드',
    items: [
      { text: '스킬 (scope·배포)', link: '/guides/skills' },
      { text: '서브 에이전트 · 오케스트레이션', link: '/guides/sub-agents' },
      { text: 'Assistants', link: '/guides/assistants' },
      { text: 'Playbooks', link: '/guides/playbooks' },
      { text: '자동화 (Scheduled Tasks)', link: '/guides/automation' },
      { text: 'Extensions (MCP)', link: '/guides/extensions' },
      { text: '내장 도구 레퍼런스', link: '/guides/builtin-tools' },
      { text: '커스텀 MCP', link: '/guides/custom-mcp' },
      { text: '세션', link: '/guides/sessions' },
      { text: '문제 해결', link: '/guides/troubleshooting' },
    ],
  },
  {
    text: 'FAQ',
    items: [
      { text: '자주 묻는 질문', link: '/faq/common-questions' },
      { text: '에러 코드', link: '/faq/error-codes' },
    ],
  },
  {
    text: '시나리오',
    items: [
      { text: '코드 리뷰', link: '/scenarios/code-review' },
      { text: '조사·리포트', link: '/scenarios/research' },
      { text: '파일 처리', link: '/scenarios/file-management' },
      { text: '웹 브라우징', link: '/scenarios/web-browsing' },
    ],
  },
];

const enSidebar = [
  {
    text: 'Get started',
    items: [
      { text: 'Overview', link: '/en/' },
      {
        text: '5-minute tutorial',
        link: '/en/getting-started/5-minute-tutorial',
      },
      { text: 'First agent chat', link: '/en/getting-started/first-agent' },
      {
        text: 'Connecting models',
        link: '/en/getting-started/connecting-models',
      },
    ],
  },
  {
    text: 'Guides',
    items: [
      { text: 'Skills', link: '/en/guides/skills' },
      { text: 'Sub-agents & orchestration', link: '/en/guides/sub-agents' },
      { text: 'Assistants', link: '/en/guides/assistants' },
      { text: 'Playbooks', link: '/en/guides/playbooks' },
      { text: 'Automation (Scheduled Tasks)', link: '/en/guides/automation' },
      { text: 'Extensions (MCP)', link: '/en/guides/extensions' },
      { text: 'Built-in Tools', link: '/en/guides/builtin-tools' },
      { text: 'Custom MCP', link: '/en/guides/custom-mcp' },
      { text: 'Sessions', link: '/en/guides/sessions' },
      { text: 'Troubleshooting', link: '/en/guides/troubleshooting' },
    ],
  },
  {
    text: 'FAQ',
    items: [
      { text: 'Common questions', link: '/en/faq/common-questions' },
      { text: 'Error codes', link: '/en/faq/error-codes' },
    ],
  },
];

export default defineConfig({
  title: 'LibrAgent',
  description:
    'LibrAgent user documentation — install, chat, and connect models',
  // Project Pages: https://fritzprix.github.io/libr-agent/
  base: '/libr-agent/',
  srcDir: '../docs/user',
  srcExclude: ['**/README.md', '**/guides/navigation-guide.md'],
  cleanUrls: true,
  lastUpdated: true,
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
  locales: {
    root: {
      label: '한국어',
      lang: 'ko-KR',
      title: 'LibrAgent',
      description: 'LibrAgent 사용자 문서 — 설치, 채팅, 모델 연결',
      themeConfig: {
        nav: koNav,
        sidebar: koSidebar,
        editLink: {
          pattern: `${repo}/edit/main/docs/user/:path`,
          text: 'GitHub에서 이 페이지 수정',
        },
        footer: {
          message: 'Released under the MIT License.',
          copyright: 'Copyright © LibrAgent contributors',
        },
        outline: { label: '이 페이지' },
        docFooter: { prev: '이전', next: '다음' },
        lastUpdated: { text: '마지막 업데이트' },
        returnToTopLabel: '맨 위로',
        sidebarMenuLabel: '메뉴',
        darkModeSwitchLabel: '테마',
        lightModeSwitchTitle: '라이트 모드',
        darkModeSwitchTitle: '다크 모드',
      },
    },
    en: {
      label: 'English',
      lang: 'en-US',
      link: '/en/',
      title: 'LibrAgent',
      description: 'LibrAgent user docs — install, chat, and connect models',
      themeConfig: {
        nav: enNav,
        sidebar: enSidebar,
        editLink: {
          pattern: `${repo}/edit/main/docs/user/:path`,
          text: 'Edit this page on GitHub',
        },
        footer: {
          message: 'Released under the MIT License.',
          copyright: 'Copyright © LibrAgent contributors',
        },
      },
    },
  },
  themeConfig: {
    socialLinks: [{ icon: 'github', link: repo }],
    search: {
      provider: 'local',
    },
  },
});
