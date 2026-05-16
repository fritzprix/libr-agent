import re

with open('src-tauri/tests/workspace_search_tests.rs', 'r') as f:
    content = f.read()

# Replace the text
content = content.replace('Use `readFile` to see the full content including the appended part.', 'Use readFile to inspect lines omitted from this preview')

with open('src-tauri/tests/workspace_search_tests.rs', 'w') as f:
    f.write(content)
