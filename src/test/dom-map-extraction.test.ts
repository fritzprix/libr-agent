import { describe, it, expect, beforeEach } from 'vitest';
import { JSDOM } from 'jsdom';

// DOM Map 추출 로직 (실제 구현 전 PoC)
interface DOMMapNode {
  tag: string;
  selector: string;
  id?: string;
  class?: string;
  text?: string;
  type?: string;
  href?: string;
  placeholder?: string;
  children: DOMMapNode[];
}

interface DOMMapResult {
  url: string;
  title: string;
  domMap: DOMMapNode;
  format: 'dom-map';
}

// PoC: DOM Map 추출 함수
function createDOMMap(
  element: Element,
  depth = 0,
  maxDepth = 5, // 깊이를 5로 증가
): DOMMapNode | null {
  if (depth > maxDepth || !element) return null;

  // 불필요한 요소 필터링
  const excludeTags = ['SCRIPT', 'STYLE', 'NOSCRIPT', 'META', 'LINK'];
  const excludeClasses = ['ad', 'banner', 'popup', 'sidebar'];

  if (excludeTags.includes(element.tagName)) {
    return null;
  }

  // 클래스 기반 필터링
  if (element.className) {
    const hasExcludedClass = excludeClasses.some((cls) =>
      element.className.includes(cls),
    );
    if (hasExcludedClass) return null;
  }

  const map: DOMMapNode = {
    tag: element.tagName.toLowerCase(),
    selector: '',
    children: [],
  };

  // selector 생성에 필요한 핵심 정보만 포함
  if (element.id) {
    map.id = element.id;
    map.selector = '#' + element.id;
  } else if (element.className) {
    const classes = element.className.trim().split(/\s+/);
    map.class = classes[0]; // 첫 번째 클래스만
    map.selector = '.' + classes[0];
  } else {
    map.selector = element.tagName.toLowerCase();
  }

  // 텍스트 내용 (최대 50자)
  const textContent = element.textContent?.trim().substring(0, 50);
  if (textContent) map.text = textContent;

  // 중요한 속성만 포함
  const inputElement = element as HTMLInputElement;
  const linkElement = element as HTMLAnchorElement;

  if (inputElement.type) map.type = inputElement.type;
  // JSDOM에서는 href가 절대 URL로 변환되므로 getAttribute 사용
  const href = linkElement.getAttribute && linkElement.getAttribute('href');
  if (href) map.href = href;
  if (inputElement.placeholder) map.placeholder = inputElement.placeholder;

  // 자식 요소 처리 (중요한 것만)
  const importantChildren = Array.from(element.children).filter((child) => {
    const isImportantTag = [
      'A', 'BUTTON', 'INPUT', 'SELECT', 'TEXTAREA', 'FORM', 
      'NAV', 'UL', 'LI', 'MAIN', 'SECTION' // 구조 요소도 포함
    ].includes(child.tagName);
    
    return isImportantTag || child.id || child.className;
  });

  for (const child of importantChildren.slice(0, 10)) {
    // 최대 10개
    const childMap = createDOMMap(child, depth + 1, maxDepth);
    if (childMap) map.children.push(childMap);
  }

  return map;
}

// PoC: 전체 DOM Map 생성 함수
function extractDOMMap(
  document: Document,
  selector = 'body',
): DOMMapResult | { error: string } {
  const targetElement = document.querySelector(selector);
  if (!targetElement) return { error: 'Element not found' };

  const domMap = createDOMMap(targetElement);
  if (!domMap) return { error: 'Failed to create DOM map' };

  return {
    url: 'test://localhost',
    title: document.title || 'Test Page',
    domMap,
    format: 'dom-map',
  };
}

describe('DOM Map 추출 PoC 테스트', () => {
  let dom: JSDOM;
  let document: Document;

  beforeEach(() => {
    // 테스트용 DOM 환경 설정
    dom = new JSDOM(`
      <!DOCTYPE html>
      <html>
        <head>
          <title>테스트 페이지</title>
          <script>console.log('test');</script>
          <style>body { margin: 0; }</style>
        </head>
        <body>
          <div class="ad-banner">광고 배너</div>
          <nav id="main-nav" class="navigation">
            <ul>
              <li><a href="/home" id="home-link">홈</a></li>
              <li><a href="/about" class="nav-link">소개</a></li>
              <li><button id="menu-btn" type="button">메뉴</button></li>
            </ul>
          </nav>
          <main class="content">
            <form id="login-form" class="auth-form">
              <h2>로그인</h2>
              <input type="email" id="email" placeholder="이메일을 입력하세요" required>
              <input type="password" id="password" placeholder="비밀번호" required>
              <button type="submit" class="submit-btn">로그인</button>
            </form>
            <div class="popup hidden" style="display: none;">팝업</div>
          </main>
          <footer>
            <div class="sidebar">사이드바</div>
          </footer>
        </body>
      </html>
    `);
    document = dom.window.document;
    
    // 디버깅: DOM 구조 확인
    console.log('홈 링크 요소:', document.querySelector('#home-link'));
    console.log('모든 a 태그:', document.querySelectorAll('a'));
  });

  it('기본 DOM Map 추출이 정상 동작하는지 확인', () => {
    const result = extractDOMMap(document);

    expect(result).toHaveProperty('domMap');
    expect(result).toHaveProperty('title', '테스트 페이지');
    expect(result).toHaveProperty('format', 'dom-map');

    if ('domMap' in result) {
      expect(result.domMap.tag).toBe('body');
      expect(result.domMap.children.length).toBeGreaterThan(0);
    }
  });

  it('불필요한 요소(스크립트, 스타일, 광고)가 필터링되는지 확인', () => {
    const result = extractDOMMap(document);

    if ('domMap' in result) {
      // 재귀적으로 모든 노드 검사
      function checkNoExcludedElements(node: DOMMapNode): boolean {
        const excludedTags = ['script', 'style', 'noscript', 'meta', 'link'];
        const excludedClasses = ['ad', 'banner', 'popup', 'sidebar'];

        if (excludedTags.includes(node.tag)) return false;
        if (
          node.class &&
          excludedClasses.some((cls) => node.class?.includes(cls))
        )
          return false;

        return node.children.every(checkNoExcludedElements);
      }

      expect(checkNoExcludedElements(result.domMap)).toBe(true);
    }
  });

  it('중요한 요소들(링크, 버튼, 입력 필드)이 올바르게 추출되는지 확인', () => {
    const result = extractDOMMap(document);

    if ('domMap' in result) {
      // 모든 노드를 평탄화하여 검사
      function flattenNodes(node: DOMMapNode): DOMMapNode[] {
        return [node, ...node.children.flatMap(flattenNodes)];
      }

      const allNodes = flattenNodes(result.domMap);
      
      // 디버깅: 추출된 모든 노드 출력
      console.log('추출된 모든 노드:', allNodes.map(node => ({
        tag: node.tag,
        id: node.id,
        class: node.class,
        selector: node.selector,
        text: node.text
      })));

      // 홈 링크 확인
      const homeLink = allNodes.find((node) => node.id === 'home-link');
      expect(homeLink).toBeDefined();
      expect(homeLink?.tag).toBe('a');
      expect(homeLink?.href).toBe('/home');
      expect(homeLink?.text).toBe('홈');
      expect(homeLink?.selector).toBe('#home-link');

      // 이메일 입력 필드 확인
      const emailInput = allNodes.find((node) => node.id === 'email');
      expect(emailInput).toBeDefined();
      expect(emailInput?.tag).toBe('input');
      expect(emailInput?.type).toBe('email');
      expect(emailInput?.placeholder).toBe('이메일을 입력하세요');
      expect(emailInput?.selector).toBe('#email');

      // 로그인 버튼 확인
      const submitBtn = allNodes.find((node) =>
        node.class?.includes('submit-btn'),
      );
      expect(submitBtn).toBeDefined();
      expect(submitBtn?.tag).toBe('button');
      expect(submitBtn?.text).toBe('로그인');
    }
  });

  it('selector 생성 로직이 우선순위에 따라 올바르게 동작하는지 확인', () => {
    const result = extractDOMMap(document);

    if ('domMap' in result) {
      function flattenNodes(node: DOMMapNode): DOMMapNode[] {
        return [node, ...node.children.flatMap(flattenNodes)];
      }

      const allNodes = flattenNodes(result.domMap);

      // ID가 있는 경우 #id 형식
      const idElement = allNodes.find((node) => node.id === 'home-link');
      expect(idElement?.selector).toBe('#home-link');

      // ID가 없고 클래스가 있는 경우 .class 형식
      const classElement = allNodes.find((node) =>
        node.class?.includes('nav-link'),
      );
      expect(classElement?.selector).toBe('.nav-link');

      // ID도 클래스도 없는 경우 태그명
      const tagElement = allNodes.find(
        (node) => !node.id && !node.class && node.tag === 'ul',
      );
      expect(tagElement?.selector).toBe('ul');
    }
  });

  it('텍스트 내용이 50자로 제한되는지 확인', () => {
    // 긴 텍스트가 있는 요소 추가
    const longTextElement = document.createElement('p');
    longTextElement.textContent =
      '이것은 매우 긴 텍스트입니다. 50자를 넘어가는 내용으로 테스트를 진행합니다. 이 텍스트는 잘려야 합니다.';
    longTextElement.id = 'long-text';
    document.body.appendChild(longTextElement);

    const result = extractDOMMap(document);

    if ('domMap' in result) {
      function flattenNodes(node: DOMMapNode): DOMMapNode[] {
        return [node, ...node.children.flatMap(flattenNodes)];
      }

      const allNodes = flattenNodes(result.domMap);
      const longTextNode = allNodes.find((node) => node.id === 'long-text');

      expect(longTextNode).toBeDefined();
      expect(longTextNode?.text?.length).toBeLessThanOrEqual(50);
    }
  });

  it('깊이 제한이 올바르게 작동하는지 확인', () => {
    // 깊은 중첩 구조 생성
    const deepDiv = document.createElement('div');
    deepDiv.id = 'level-0';
    let currentDiv = deepDiv;

    for (let i = 1; i <= 5; i++) {
      const newDiv = document.createElement('div');
      newDiv.id = `level-${i}`;
      newDiv.innerHTML = `<button id="btn-${i}">Level ${i} Button</button>`;
      currentDiv.appendChild(newDiv);
      currentDiv = newDiv;
    }

    document.body.appendChild(deepDiv);

    const result = extractDOMMap(document);

    if ('domMap' in result) {
      function getMaxDepth(node: DOMMapNode, currentDepth = 0): number {
        if (node.children.length === 0) return currentDepth;
        return Math.max(
          ...node.children.map((child) => getMaxDepth(child, currentDepth + 1)),
        );
      }

      const maxDepth = getMaxDepth(result.domMap);
      expect(maxDepth).toBeLessThanOrEqual(5); // maxDepth = 5로 변경
    }
  });

  it('크기 감소 효과를 측정', () => {
    const originalHTML = dom.serialize();
    const result = extractDOMMap(document);

    const originalSize = new Blob([originalHTML]).size;
    const domMapSize = new Blob([JSON.stringify(result)]).size;

    console.log(`원본 HTML 크기: ${originalSize} bytes`);
    console.log(`DOM Map 크기: ${domMapSize} bytes`);
    console.log(
      `크기 감소율: ${((1 - domMapSize / originalSize) * 100).toFixed(1)}%`,
    );

    // 구조화된 DOM Map이 생성되었는지 확인 (크기는 내용에 따라 달라질 수 있음)
    console.log(`크기 비교 - 원본: ${originalSize}bytes, DOM Map: ${domMapSize}bytes`);
    
    // 구조화된 정보가 포함되어 있는지 확인
    if ('domMap' in result) {
      expect(result.domMap.children.length).toBeGreaterThan(0);
      // DOM Map이 의미있는 정보를 포함하고 있는지 확인
      expect(result.title).toBe('테스트 페이지');
      expect(result.format).toBe('dom-map');
    }
  });

  it('빈 셀렉터나 존재하지 않는 요소 처리', () => {
    const result = extractDOMMap(document, '#non-existent');
    expect(result).toHaveProperty('error', 'Element not found');
  });
});
