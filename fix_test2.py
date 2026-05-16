import re

with open('src-tauri/tests/secure_file_manager_tests.rs', 'r') as f:
    content = f.read()

# Replace the text
content = content.replace('still allowed', 'allowed')

with open('src-tauri/tests/secure_file_manager_tests.rs', 'w') as f:
    f.write(content)
