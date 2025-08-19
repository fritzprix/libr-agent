## \#\# Refactoring Plan: 미구현 Browser Agent 기능 완성

### \#\#\# 1. 작업의 목적

  - `take_screenshot`, `get_page_performance` 등 현재 플레이스홀더(Placeholder)로 남아있는 브라우저 에이전트의 **핵심 기능들을 `execute_script`를 활용하여 완전히 구현**합니다.
  - 모든 브라우저 관련 기능이 일관된 JavaScript 실행 및 결과 반환 구조를 따르도록 하여 **코드의 통일성과 유지보수성을 향상**시킵니다.

-----

### \#\#\# 2. 현재의 상태 / 문제점

  - \*\*`interactive_browser_server.rs`\*\*의 여러 함수들(`take_screenshot`, `get_page_performance`, `get_page_images`, `get_page_links` 등)이 실제 데이터를 추출하는 대신, **고정된 메시지나 제한된 정보만을 담은 JSON 문자열을 반환**하고 있습니다. [cite: interactive\_browser\_server.rs]
  - 예를 들어, `take_screenshot` 함수는 실제 스크린샷을 찍는 대신 "Screenshot functionality requires html2canvas library..." 라는 메시지만을 반환합니다. [cite: interactive\_browser\_server.rs]
  - 이로 인해 `browser_agent` 도구의 기능이 불완전하며, 사용자나 AI 모델이 웹페이지에 대한 상세 정보를 얻을 수 없습니다.

-----

### \#\#\# 3. 변경 이후의 상태 / 해결 판정 기준

  - `take_screenshot` 호출 시, 페이지의 기본 정보(뷰포트, URL 등)가 담긴 **JSON 객체를 정상적으로 반환**합니다. (실제 이미지 캡처는 `html2canvas` 같은 외부 라이브러리 주입이 필요하므로, 이번 단계에서는 정보 반환을 목표로 합니다.)
  - `get_page_performance`, `get_page_images`, `get_page_links` 등의 함수들이 각각 **실제 성능 지표, 이미지 목록, 링크 목록을 추출하여 JSON 형식으로 반환**합니다.
  - **해결 판정 기준:** 각 도구를 호출했을 때, 플레이스홀더 메시지 대신 실제 추출된 데이터가 포함된 성공 응답을 MCP(Model Context Protocol)를 통해 수신하는 것을 확인합니다.

-----

### \#\#\# 4. 수정이 필요한 코드 및 수정부분의 코드 스니핏

#### **1. `interactive_browser_server.rs`: `take_screenshot` 함수 구현**

  * 실제 이미지 캡처 대신, 페이지의 현재 상태를 파악할 수 있는 유용한 정보를 반환하도록 스크립트를 작성하여 `execute_script`로 실행합니다.

<!-- end list -->

```rust
// interactive_browser_server.rs

pub async fn take_screenshot(&self, session_id: &str, timeout_secs: Option<u64>) -> Result<String, String> {
    debug!("Taking screenshot info for session {}", session_id);

    let script = r#"
        (function() {
            try {
                // 실제 이미지 캡처는 html2canvas 같은 외부 라이브러리 주입이 필요하므로,
                // 이 단계에서는 페이지의 현재 상태 정보를 반환합니다.
                return JSON.stringify({
                    type: 'screenshot_info',
                    viewport: {
                        width: window.innerWidth,
                        height: window.innerHeight,
                        devicePixelRatio: window.devicePixelRatio
                    },
                    url: window.location.href,
                    title: document.title,
                    timestamp: new Date().toISOString()
                });
            } catch (error) {
                return JSON.stringify({ error: 'Screenshot info failed: ' + error.message });
            }
        })()
    "#;

    self.execute_script(session_id, script, timeout_secs).await
}
```

-----

#### **2. `interactive_browser_server.rs`: `get_page_performance` 함수 구현**

  * `window.performance` API를 활용하여 페이지의 상세 성능 지표를 수집하는 스크립트를 구현합니다.

<!-- end list -->

```rust
// interactive_browser_server.rs

pub async fn get_page_performance(&self, session_id: &str, timeout_secs: Option<u64>) -> Result<String, String> {
    debug!("Getting page performance metrics for session {}", session_id);

    let script = r#"
        (function() {
            try {
                const performance = window.performance;
                if (!performance) {
                    return JSON.stringify({ error: 'Performance API not supported.' });
                }
                const navigation = performance.getEntriesByType('navigation')[0];
                const paint = performance.getEntriesByType('paint');
                const metrics = {
                    loadTime: navigation ? navigation.duration : null,
                    domContentLoadedTime: navigation ? navigation.domContentLoadedEventEnd : null,
                    firstContentfulPaint: paint.find(p => p.name === 'first-contentful-paint')?.startTime || null
                };
                return JSON.stringify(metrics);
            } catch (error) {
                return JSON.stringify({ error: 'Failed to get performance metrics: ' + error.message });
            }
        })()
    "#;

    self.execute_script(session_id, script, timeout_secs).await
}
```

-----

#### **3. `interactive_browser_server.rs`: `get_page_images` 및 `get_page_links` 함수 구현**

  * `document.querySelectorAll`을 사용하여 페이지 내의 모든 이미지(`<img>`)와 링크(`<a>`) 태그를 찾아 정보를 추출합니다.

<!-- end list -->

```rust
// interactive_browser_server.rs

pub async fn get_page_images(&self, session_id: &str, timeout_secs: Option<u64>) -> Result<String, String> {
    debug!("Getting page images for session {}", session_id);
    let script = r#"
        (function() {
            const images = Array.from(document.querySelectorAll('img')).map(img => ({
                src: img.src,
                alt: img.alt,
                width: img.naturalWidth,
                height: img.naturalHeight
            }));
            return JSON.stringify({ count: images.length, images: images });
        })()
    "#;
    self.execute_script(session_id, script, timeout_secs).await
}

pub async fn get_page_links(&self, session_id: &str, timeout_secs: Option<u64>) -> Result<String, String> {
    debug!("Getting page links for session {}", session_id);
    let script = r#"
        (function() {
            const links = Array.from(document.querySelectorAll('a[href]')).map(a => ({
                href: a.href,
                text: a.textContent.trim()
            }));
            return JSON.stringify({ count: links.length, links: links });
        })()
    "#;
    self.execute_script(session_id, script, timeout_secs).await
}
```