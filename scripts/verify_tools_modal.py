import argparse
import importlib
import os
from pathlib import Path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description='Capture Agent Tools modal screenshots with mocked Tauri IPC.',
    )
    parser.add_argument(
        '--base-url',
        default='http://localhost:1420',
        help='Frontend base URL (default: http://localhost:1420)',
    )
    parser.add_argument(
        '--session-id',
        default='session-123',
        help='Agent session ID used in route and IPC mock (default: session-123)',
    )
    parser.add_argument(
        '--output-dir',
        default=os.environ.get('TOOLS_MODAL_OUTPUT_DIR', 'playwright-report/tools-modal'),
        help='Directory for screenshots (default: TOOLS_MODAL_OUTPUT_DIR or playwright-report/tools-modal)',
    )
    return parser.parse_args()


def build_tauri_mock_script(session_id: str) -> str:
    return f"""
      window.__TAURI__ = {{
        core: {{
          invoke: async (cmd, args) => {{
            console.log('Invoke:', cmd, args);
            if (cmd === 'plugin:store|get') {{
              return null;
            }}
            if (cmd === 'agent_list_sessions') {{
              return {{ items: [], total: 0, page: 1, pageSize: 10 }};
            }}
            if (cmd === 'agent_get_session') {{
              return {{
                id: '{session_id}',
                title: 'Test Session',
                created_at: new Date().toISOString(),
                updated_at: new Date().toISOString(),
                agent_id: 'agent-1',
                model: 'gpt-4o',
                provider: 'openai',
              }};
            }}
            if (cmd === 'agent_get_available_tools' || cmd === 'agent_get_tools') {{
              return [
                {{
                  name: 'builtin_tool_1',
                  description: 'A built-in tool',
                  inputSchema: {{ type: 'object', properties: {{ arg: {{ type: 'string' }} }} }}
                }},
                {{
                  name: 'mcp_tool_1',
                  description: 'An MCP tool',
                  inputSchema: {{ type: 'object', properties: {{ arg: {{ type: 'string' }} }} }}
                }}
              ];
            }}
            if (cmd === 'messages_get_page') {{
              return {{ items: [], total: 0, page: 1, pageSize: 50 }};
            }}
            if (cmd === 'plugin:event|listen') {{
              return 1;
            }}
            return null;
          }}
        }}
      }};
      window.__TAURI_INTERNALS__ = {{
        invoke: window.__TAURI__.core.invoke,
        transformCallback: (callback) => callback
      }};
    """


def verify_tools_modal(base_url: str, session_id: str, output_dir: Path) -> None:
  try:
    playwright_sync_api = importlib.import_module('playwright.sync_api')
    playwright_timeout_error = getattr(playwright_sync_api, 'TimeoutError')
    sync_playwright = getattr(playwright_sync_api, 'sync_playwright')
  except (ModuleNotFoundError, AttributeError) as error:
    raise RuntimeError(
      'Playwright is not installed. Install it with: pip install playwright '
      'and then run: playwright install'
    ) from error

  output_dir.mkdir(parents=True, exist_ok=True)
  main_page_path = output_dir / 'main_page.png'
  tools_modal_path = output_dir / 'tools_modal.png'
  error_path = output_dir / 'error.png'

  with sync_playwright() as playwright:
    browser = playwright.chromium.launch(headless=True)
    context = browser.new_context(viewport={'width': 1280, 'height': 720})
    page = context.new_page()

    page.add_init_script(build_tauri_mock_script(session_id))

    try:
      page.goto(f'{base_url}/agent/{session_id}')
      page.wait_for_load_state('domcontentloaded')

      tools_button = page.locator('button[title*="view available tools" i]')
      if tools_button.count() == 0:
        tools_button = page.locator('button:has(svg.lucide-wrench)')

      tools_button.first.wait_for(state='visible', timeout=5000)
      page.screenshot(path=str(main_page_path))

      tools_button.first.click()
      page.get_by_role('dialog').wait_for(state='visible', timeout=5000)
      page.get_by_role('heading', name='Available Tools').wait_for(
        state='visible',
        timeout=5000,
      )

      page.screenshot(path=str(tools_modal_path))
      print(f'Captured screenshots in: {output_dir}')

    except playwright_timeout_error as error:  # type: ignore[misc]
      print(f'Timeout while verifying tools modal: {error}')
      page.screenshot(path=str(error_path))
      raise
    except Exception as error:
      print(f'Error while verifying tools modal: {error}')
      page.screenshot(path=str(error_path))
      raise
    finally:
      browser.close()


if __name__ == '__main__':
    args = parse_args()
    verify_tools_modal(
        base_url=args.base_url,
        session_id=args.session_id,
        output_dir=Path(args.output_dir),
    )
