const fs = require('fs');

const enPath = 'src/locales/en/common.json';
const koPath = 'src/locales/ko/common.json';

const enData = JSON.parse(fs.readFileSync(enPath, 'utf8'));
const koData = JSON.parse(fs.readFileSync(koPath, 'utf8'));

if (!enData.agent) enData.agent = {};
if (!enData.agent.toolsModal) enData.agent.toolsModal = {};

Object.assign(enData.agent.toolsModal, {
  title: "Available Tools",
  subtitleWithCounts: "Built-in tools: {{builtinCount}} • MCP tools: {{mcpCount}}",
  subtitleDefault: "List of tools available to this agent session.",
  loading: "Loading tools...",
  errorTitle: "Error loading tools",
  empty: "No tools available for this agent session.",
  ariaLabel: "Available tools list",
  badgeBuiltin: "builtin",
  badgeMcp: "mcp",
  viewSchema: "View Input Schema"
});

if (!koData.agent) koData.agent = {};
if (!koData.agent.toolsModal) koData.agent.toolsModal = {};

Object.assign(koData.agent.toolsModal, {
  title: "사용 가능한 도구",
  subtitleWithCounts: "내장 도구: {{builtinCount}} • MCP 도구: {{mcpCount}}",
  subtitleDefault: "이 에이전트 세션에서 사용할 수 있는 도구 목록입니다.",
  loading: "도구 로딩 중...",
  errorTitle: "도구 로딩 오류",
  empty: "이 에이전트 세션에 사용할 수 있는 도구가 없습니다.",
  ariaLabel: "사용 가능한 도구 목록",
  badgeBuiltin: "내장",
  badgeMcp: "mcp",
  viewSchema: "입력 스키마 보기"
});

fs.writeFileSync(enPath, JSON.stringify(enData, null, 2) + '\n');
fs.writeFileSync(koPath, JSON.stringify(koData, null, 2) + '\n');
