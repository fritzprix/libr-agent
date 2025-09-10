import { createUIResource, UIResource } from '@mcp-ui/server';

// UIResource 유형 정의
export interface MockUIResource {
  id: string;
  title: string;
  description: string;
  resource: {
    type: 'resource';
    resource: {
      uri: string;
      mimeType: string;
      text?: string;
      blob?: string;
    };
  };
}

// Mock MCP Server 클래스
export class MockMCPServer {
  private resources: MockUIResource[] = [];

  constructor() {
    this.initializeResources();
  }

  private initializeResources() {
    // 1. Raw HTML Resource
    const htmlResource = createUIResource({
      uri: 'ui://test/html-component',
      content: {
        type: 'rawHtml',
        htmlString: `
          <div style="padding: 20px; border: 2px solid #3b82f6; border-radius: 8px; background: #f8fafc;">
            <h2 style="color: #1e40af; margin-bottom: 10px;">HTML Resource Test</h2>
            <p style="margin-bottom: 15px;">This is a raw HTML resource rendered inside an iframe.</p>
            <button 
              onclick="window.parent.postMessage({type: 'tool', payload: {toolName: 'htmlButtonClick', params: {source: 'html'}}}, '*')"
              style="background: #3b82f6; color: white; padding: 8px 16px; border: none; border-radius: 4px; cursor: pointer;"
            >
              Click Me (HTML)
            </button>
            <div style="margin-top: 15px; padding: 10px; background: #e0f2fe; border-radius: 4px;">
              <small>URI: ui://test/html-component</small>
            </div>
          </div>
        `,
      },
      encoding: 'text',
    });

    this.resources.push({
      id: 'html-1',
      title: 'Raw HTML Resource',
      description: 'A basic HTML component with interactive button',
      resource: htmlResource,
    });

    // 2. External URL Resource
    const urlResource = createUIResource({
      uri: 'ui://test/external-url',
      content: {
        type: 'externalUrl',
        iframeUrl: 'https://jsonplaceholder.typicode.com/',
      },
      encoding: 'text',
    });

    this.resources.push({
      id: 'url-1',
      title: 'External URL Resource',
      description: 'External website loaded in iframe',
      resource: urlResource,
    });

    // 3. Form HTML Resource
    const formResource = createUIResource({
      uri: 'ui://test/form-component',
      content: {
        type: 'rawHtml',
        htmlString: `
          <div style="padding: 24px; max-width: 400px; background: #ffffff; border: 1px solid #e5e7eb; border-radius: 8px; box-shadow: 0 1px 3px rgba(0,0,0,0.1);">
            <h3 style="color: #111827; margin-bottom: 16px; font-size: 18px;">User Information Form</h3>
            <form id="userForm">
              <div style="margin-bottom: 16px;">
                <label style="display: block; margin-bottom: 4px; font-weight: 500; color: #374151;">Name:</label>
                <input type="text" id="name" name="name" style="width: 100%; padding: 8px 12px; border: 1px solid #d1d5db; border-radius: 4px;" placeholder="Enter your name">
              </div>
              <div style="margin-bottom: 16px;">
                <label style="display: block; margin-bottom: 4px; font-weight: 500; color: #374151;">Email:</label>
                <input type="email" id="email" name="email" style="width: 100%; padding: 8px 12px; border: 1px solid #d1d5db; border-radius: 4px;" placeholder="Enter your email">
              </div>
              <div style="margin-bottom: 20px;">
                <label style="display: block; margin-bottom: 4px; font-weight: 500; color: #374151;">Message:</label>
                <textarea id="message" name="message" rows="3" style="width: 100%; padding: 8px 12px; border: 1px solid #d1d5db; border-radius: 4px;" placeholder="Enter your message"></textarea>
              </div>
              <button type="submit" style="width: 100%; background: #10b981; color: white; padding: 10px; border: none; border-radius: 4px; font-weight: 500; cursor: pointer;">
                Submit Form
              </button>
            </form>
            
            <script>
              document.getElementById('userForm').addEventListener('submit', function(e) {
                e.preventDefault();
                const formData = {
                  name: document.getElementById('name').value,
                  email: document.getElementById('email').value,
                  message: document.getElementById('message').value
                };
                window.parent.postMessage({
                  type: 'tool',
                  payload: {
                    toolName: 'submitForm',
                    params: formData
                  }
                }, '*');
              });
            </script>
          </div>
        `,
      },
      encoding: 'text',
    });

    this.resources.push({
      id: 'form-1',
      title: 'Interactive Form',
      description: 'A form that collects user data and sends it via tool call',
      resource: formResource,
    });

    const remoteDomScript = `
let isDarkMode = false;

// Create the main container stack with centered alignment
const stack = document.createElement('ui-stack');
stack.setAttribute('direction', 'vertical');
stack.setAttribute('spacing', '20');
stack.setAttribute('align', 'center');

// Create the title text
const title = document.createElement('ui-text');
title.setAttribute('content', 'Logo Toggle Demo');

// Create a centered container for the logo
const logoContainer = document.createElement('ui-stack');
logoContainer.setAttribute('direction', 'vertical');
logoContainer.setAttribute('spacing', '0');
logoContainer.setAttribute('align', 'center');

// Create the logo image (starts with light theme)
const logo = document.createElement('ui-image');
logo.setAttribute('src', 'https://block.github.io/goose/img/logo_light.png');
logo.setAttribute('alt', 'Goose Logo');
logo.setAttribute('width', '200');

// Create the toggle button
const toggleButton = document.createElement('ui-button');
toggleButton.setAttribute('label', '🌙 Switch to Dark Mode');

// Add the toggle functionality
toggleButton.addEventListener('press', () => {
  isDarkMode = !isDarkMode;
  
  if (isDarkMode) {
    // Switch to dark mode
    logo.setAttribute('src', 'https://block.github.io/goose/img/logo_dark.png');
    logo.setAttribute('alt', 'Goose Logo (Dark Mode)');
    toggleButton.setAttribute('label', '☀️ Switch to Light Mode');
  } else {
    // Switch to light mode
    logo.setAttribute('src', 'https://block.github.io/goose/img/logo_light.png');
    logo.setAttribute('alt', 'Goose Logo (Light Mode)');
    toggleButton.setAttribute('label', '🌙 Switch to Dark Mode');
  }
  
  console.log('Logo toggled to:', isDarkMode ? 'dark' : 'light', 'mode');
});

// Assemble the UI
logoContainer.appendChild(logo);
stack.appendChild(title);
stack.appendChild(logoContainer);
stack.appendChild(toggleButton);
root.appendChild(stack);
`;

    // 4. Remote DOM Resource (React)

    const remoteDomResource = createUIResource({
      uri: 'ui://test/remote-dom-component',
      content: {
        type: 'remoteDom',
        script: remoteDomScript,
        framework: 'react',
      },
      encoding: 'text',
    });

    this.resources.push({
      id: 'remote-dom-1',
      title: 'Remote DOM Component',
      description: 'A component built with Remote DOM technology',
      resource: remoteDomResource,
    });

    const simpleRemoteDomScript = `
console.log('Remote DOM script starting...');
console.log('Available globals:', Object.keys(window));

const button = document.createElement('button');
button.setAttribute('label', 'Click me!');
button.addEventListener('click', () => {
    console.log('Button clicked!');
    window.parent.postMessage({
        type: 'tool',
        payload: {
          toolName: 'submitName',
          params: { name: 'david' }
        }
    }, '*');
});

root.appendChild(button);
console.log('Button added to root:', root, button);
console.log('Root children:', root.children);

// MutationObserver 상태 확인
console.log('MutationObserver available:', typeof MutationObserver);

// 5초 후 상태 확인
setTimeout(() => {
    console.log('5 seconds later:');
    console.log('- button still exists:', root.contains(button));
    console.log('- root children count:', root.children.length);
    console.log('- root innerHTML:', root.innerHTML);
}, 5000);
`;

    const simpleRemoteDomResource: UIResource = createUIResource({
      uri: 'ui://test/simple-remote-dom-component',
      content: {
        script: simpleRemoteDomScript,
        framework: 'webcomponents',
        type: 'remoteDom',
      },
      encoding: 'blob',
    });

    this.resources.push({
      id: 'remote-dom-2',
      description: 'simplest form of remote dom',
      resource: simpleRemoteDomResource,
      title: 'Simple Remote Dom',
    });

    // 5. Complex Interactive Dashboard
    const dashboardResource = createUIResource({
      uri: 'ui://test/dashboard',
      content: {
        type: 'rawHtml',
        htmlString: `
          <div style="padding: 24px; background: linear-gradient(135deg, #667eea 0%, #764ba2 100%); border-radius: 12px; color: white;">
            <h2 style="margin-bottom: 20px; font-size: 24px;">Interactive Dashboard</h2>
            
            <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 16px; margin-bottom: 24px;">
              <div style="background: rgba(255,255,255,0.1); padding: 16px; border-radius: 8px; backdrop-filter: blur(10px);">
                <h4 style="margin-bottom: 8px;">Total Users</h4>
                <div style="font-size: 28px; font-weight: bold;" id="userCount">1,234</div>
              </div>
              <div style="background: rgba(255,255,255,0.1); padding: 16px; border-radius: 8px; backdrop-filter: blur(10px);">
                <h4 style="margin-bottom: 8px;">Revenue</h4>
                <div style="font-size: 28px; font-weight: bold;" id="revenue">$45,678</div>
              </div>
              <div style="background: rgba(255,255,255,0.1); padding: 16px; border-radius: 8px; backdrop-filter: blur(10px);">
                <h4 style="margin-bottom: 8px;">Growth</h4>
                <div style="font-size: 28px; font-weight: bold; color: #4ade80;" id="growth">+12.5%</div>
              </div>
            </div>
            
            <div style="display: flex; gap: 12px; flex-wrap: wrap;">
              <button onclick="refreshData()" style="background: rgba(255,255,255,0.2); color: white; border: 1px solid rgba(255,255,255,0.3); padding: 8px 16px; border-radius: 6px; cursor: pointer;">
                Refresh Data
              </button>
              <button onclick="exportData()" style="background: rgba(255,255,255,0.2); color: white; border: 1px solid rgba(255,255,255,0.3); padding: 8px 16px; border-radius: 6px; cursor: pointer;">
                Export Report
              </button>
              <button onclick="openSettings()" style="background: rgba(255,255,255,0.2); color: white; border: 1px solid rgba(255,255,255,0.3); padding: 8px 16px; border-radius: 6px; cursor: pointer;">
                Settings
              </button>
            </div>
          </div>
          
          <script>
            function refreshData() {
              document.getElementById('userCount').textContent = Math.floor(Math.random() * 10000);
              document.getElementById('revenue').textContent = '$' + Math.floor(Math.random() * 100000).toLocaleString();
              document.getElementById('growth').textContent = (Math.random() * 20 - 5).toFixed(1) + '%';
              
              window.parent.postMessage({
                type: 'notify',
                payload: { message: 'Dashboard data refreshed successfully!' }
              }, '*');
            }
            
            function exportData() {
              window.parent.postMessage({
                type: 'tool',
                payload: {
                  toolName: 'exportDashboard',
                  params: { format: 'pdf', timestamp: new Date().toISOString() }
                }
              }, '*');
            }
            
            function openSettings() {
              window.parent.postMessage({
                type: 'intent',
                payload: {
                  intent: 'openSettings',
                  params: { section: 'dashboard' }
                }
              }, '*');
            }
          </script>
        `,
      },
      encoding: 'text',
    });

    this.resources.push({
      id: 'dashboard-1',
      title: 'Interactive Dashboard',
      description: 'A complex dashboard with multiple UI actions',
      resource: dashboardResource,
    });
  }

  // 모든 리소스 가져오기
  getAllResources(): MockUIResource[] {
    return this.resources;
  }

  // ID로 리소스 가져오기
  getResourceById(id: string): MockUIResource | undefined {
    return this.resources.find((res) => res.id === id);
  }

  // 리소스 개수
  getResourceCount(): number {
    return this.resources.length;
  }

  // 특정 타입의 리소스들만 가져오기
  getResourcesByType(
    type: 'rawHtml' | 'externalUrl' | 'remoteDom',
  ): MockUIResource[] {
    return this.resources.filter((res) => {
      const content = res.resource.resource;
      if (content.mimeType === 'text/html') return type === 'rawHtml';
      if (content.mimeType === 'text/uri-list') return type === 'externalUrl';
      if (content.mimeType?.includes('remote-dom')) return type === 'remoteDom';
      return false;
    });
  }
}

// 싱글톤 인스턴스
export const mockMCPServer = new MockMCPServer();
