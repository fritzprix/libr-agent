#!/usr/bin/env python3
"""
Add threadId field to all Message objects in test files.
"""

import re
import sys
from pathlib import Path

def add_threadid_to_message_objects(file_path):
    """Add threadId to Message objects that have sessionId but no threadId."""
    with open(file_path, 'r') as f:
        content = f.read()
    
    # Pattern to match sessionId line and capture the value
    # Looks for: sessionId: 'value' or sessionId: "value"
    pattern = r"(\s+)sessionId: (['\"])(.+?)\2,(\s*\n)(?!.*threadId)"
    
    def replacer(match):
        indent = match.group(1)
        quote = match.group(2)
        session_value = match.group(3)
        newline = match.group(4)
        return f"{indent}sessionId: {quote}{session_value}{quote},{newline}{indent}threadId: {quote}{session_value}{quote},{newline}"
    
    new_content = re.sub(pattern, replacer, content)
    
    if new_content != content:
        with open(file_path, 'w') as f:
            f.write(new_content)
        return True
    return False

def main():
    test_dir = Path('src/lib/ai-service/__tests__')
    
    if not test_dir.exists():
        print(f"Test directory not found: {test_dir}")
        return 1
    
    modified_count = 0
    for test_file in test_dir.glob('**/*.test.ts'):
        if add_threadid_to_message_objects(test_file):
            print(f"Modified: {test_file}")
            modified_count += 1
    
    print(f"\nTotal files modified: {modified_count}")
    return 0

if __name__ == '__main__':
    sys.exit(main())
