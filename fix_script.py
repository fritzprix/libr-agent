import re

with open('src-tauri/src/mcp/builtin/workspace/file_operations/write.rs', 'r') as f:
    lines = f.readlines()

new_lines = []
for line in lines:
    new_lines.append(line)

with open('src-tauri/src/mcp/builtin/workspace/file_operations/write.rs', 'w') as f:
    f.writelines(new_lines)
