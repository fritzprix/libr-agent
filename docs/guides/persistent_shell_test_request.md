# Persistent Shell Feature Test Request for AI Agent

**Date**: 2025-11-22  
**Feature**: STDIO-based Persistent Shell Integration  
**Target**: LibrAgent Workspace Tool  
**Test Goal**: Verify state preservation and performance improvements

---

## 📋 Test Overview

This document provides test scenarios for validating the newly implemented persistent shell feature in LibrAgent's workspace tool. The persistent shell maintains state across multiple commands, enabling working directory preservation, environment variables, and virtual environment activation.

---

## 🎯 Test Scenarios

### Test 1: Working Directory State Preservation

**Objective**: Verify that `cd` command state persists across multiple executions.

**Commands**:

```bash
# Step 1: Change to a temporary directory
cd /tmp

# Step 2: Verify current directory (should be /tmp)
pwd

# Step 3: Create a test file
echo "test content" > test_persistent_shell.txt

# Step 4: List files (should show test_persistent_shell.txt)
ls -la test_persistent_shell.txt

# Step 5: Navigate to subdirectory
mkdir -p subdir && cd subdir

# Step 6: Verify nested directory
pwd
```

**Windows Alternative**:

```powershell
# Step 1: Change to temp directory
cd $env:TEMP

# Step 2: Verify current directory
Get-Location

# Step 3: Create a test file
"test content" | Out-File test_persistent_shell.txt

# Step 4: List files
Get-ChildItem test_persistent_shell.txt

# Step 5: Navigate to subdirectory
New-Item -ItemType Directory -Force -Path subdir; cd subdir

# Step 6: Verify nested directory
Get-Location
```

**Expected Results**:

- Step 2: Output shows `/tmp` (Unix) or `C:\Users\...\AppData\Local\Temp` (Windows)
- Step 4: File exists and is listed
- Step 6: Output shows `/tmp/subdir` (Unix) or temp directory with `\subdir` (Windows)

---

### Test 2: Environment Variable Persistence

**Objective**: Verify that environment variables persist across commands.

**Commands (Unix)**:

```bash
# Step 1: Set environment variable
export MY_TEST_VAR="PersistentShellWorks"

# Step 2: Verify variable is set
echo $MY_TEST_VAR

# Step 3: Use variable in another command
echo "The value is: $MY_TEST_VAR"

# Step 4: Set multiple variables
export VAR1="value1" VAR2="value2"

# Step 5: Verify both variables
echo "VAR1=$VAR1, VAR2=$VAR2"
```

**Commands (Windows)**:

```powershell
# Step 1: Set environment variable
$env:MY_TEST_VAR = "PersistentShellWorks"

# Step 2: Verify variable is set
Write-Output $env:MY_TEST_VAR

# Step 3: Use variable in another command
Write-Output "The value is: $env:MY_TEST_VAR"

# Step 4: Set multiple variables
$env:VAR1 = "value1"; $env:VAR2 = "value2"

# Step 5: Verify both variables
Write-Output "VAR1=$env:VAR1, VAR2=$env:VAR2"
```

**Expected Results**:

- Step 2: Output shows `PersistentShellWorks`
- Step 3: Output shows `The value is: PersistentShellWorks`
- Step 5: Output shows `VAR1=value1, VAR2=value2`

---

### Test 3: Python Virtual Environment Activation

**Objective**: Verify that Python virtual environment state persists.

**Setup (Unix)**:

```bash
# Create virtual environment
cd /tmp
python3 -m venv test_venv

# Activate venv
source test_venv/bin/activate

# Verify activation
which python
python --version

# Install package in venv
pip install requests

# Verify package installed
pip list | grep requests

# Use installed package
python -c "import requests; print(requests.__version__)"
```

**Setup (Windows)**:

```powershell
# Create virtual environment
cd $env:TEMP
python -m venv test_venv

# Activate venv
.\test_venv\Scripts\Activate.ps1

# Verify activation
Get-Command python | Select-Object -ExpandProperty Source
python --version

# Install package in venv
pip install requests

# Verify package installed
pip list | Select-String requests

# Use installed package
python -c "import requests; print(requests.__version__)"
```

**Expected Results**:

- `which python` / `Get-Command python` shows path inside `test_venv`
- `pip list` shows `requests` package
- Python successfully imports and uses `requests`

---

### Test 4: Complex Workflow (Multi-step Build)

**Objective**: Verify persistent shell handles real-world multi-step workflows.

**Scenario**: Node.js project setup and build

**Commands (Unix)**:

```bash
# Step 1: Create project directory
cd /tmp
mkdir -p test_node_project
cd test_node_project

# Step 2: Initialize npm project
npm init -y

# Step 3: Install dependencies
npm install express

# Step 4: Create simple server
cat > server.js << 'EOF'
const express = require('express');
const app = express();
app.get('/', (req, res) => res.send('Hello from persistent shell!'));
console.log('Server ready on port 3000');
EOF

# Step 5: Verify file created
ls -la server.js

# Step 6: Check node_modules
ls -d node_modules/express
```

**Commands (Windows)**:

```powershell
# Step 1: Create project directory
cd $env:TEMP
New-Item -ItemType Directory -Force -Path test_node_project
cd test_node_project

# Step 2: Initialize npm project
npm init -y

# Step 3: Install dependencies
npm install express

# Step 4: Create simple server
@"
const express = require('express');
const app = express();
app.get('/', (req, res) => res.send('Hello from persistent shell!'));
console.log('Server ready on port 3000');
"@ | Out-File -Encoding utf8 server.js

# Step 5: Verify file created
Get-ChildItem server.js

# Step 6: Check node_modules
Test-Path node_modules/express
```

**Expected Results**:

- All commands execute in same directory context
- `node_modules` and `package.json` exist
- `server.js` file created successfully

---

### Test 5: Error Handling and Recovery

**Objective**: Verify persistent shell handles errors gracefully.

**Commands (Unix)**:

```bash
# Step 1: Execute failing command
ls /nonexistent_directory_12345

# Step 2: Verify shell still works after error
echo "Shell is still alive"

# Step 3: Check exit code handling
cd /nonexistent_path_67890 || echo "Command failed as expected"

# Step 4: Continue with valid commands
pwd
```

**Commands (Windows)**:

```powershell
# Step 1: Execute failing command
Get-ChildItem C:\nonexistent_directory_12345

# Step 2: Verify shell still works after error
Write-Output "Shell is still alive"

# Step 3: Check exit code handling
try { Set-Location C:\nonexistent_path_67890 } catch { Write-Output "Command failed as expected" }

# Step 4: Continue with valid commands
Get-Location
```

**Expected Results**:

- Step 1: Error message in stderr, non-zero exit code
- Step 2: Output shows "Shell is still alive"
- Step 3: Error caught gracefully
- Step 4: Valid output, shell continues working

---

### Test 6: Performance Comparison

**Objective**: Demonstrate performance improvement over one-shot execution.

**Test A: Persistent Shell (Default)**:

```bash
# Execute 10 simple commands sequentially
echo "Command 1"
echo "Command 2"
echo "Command 3"
echo "Command 4"
echo "Command 5"
echo "Command 6"
echo "Command 7"
echo "Command 8"
echo "Command 9"
echo "Command 10"
```

**Test B: One-shot Mode (for comparison)**:

```json
{
  "command": "echo 'Command 1'",
  "use_persistent_shell": false
}
```

(Repeat 10 times with different commands)

**Expected Results**:

- Test A: All 10 commands complete in ~200-500ms total
- Test B: Each command takes ~50-100ms, total ~500-1000ms
- Persistent shell should be **2-3x faster** for sequential commands

---

### Test 7: Session Isolation

**Objective**: Verify different sessions have independent shell states.

**Session A Commands**:

```bash
export SESSION_VAR="Session A"
cd /tmp/session_a
mkdir -p /tmp/session_a
echo $SESSION_VAR
pwd
```

**Session B Commands** (in different session):

```bash
export SESSION_VAR="Session B"
cd /tmp/session_b
mkdir -p /tmp/session_b
echo $SESSION_VAR
pwd
```

**Verification**:

- Session A should always see `SESSION_VAR="Session A"` and `/tmp/session_a`
- Session B should always see `SESSION_VAR="Session B"` and `/tmp/session_b`
- No cross-contamination between sessions

---

### Test 8: UTF-8 and Special Characters

**Objective**: Verify proper handling of non-ASCII characters and error messages.

**Commands (Windows - Korean locale test)**:

```powershell
# Step 1: Create file with Korean name
"테스트" | Out-File -Encoding utf8 한글파일.txt

# Step 2: List Korean filename
Get-ChildItem 한글파일.txt

# Step 3: Trigger Korean error message
Get-ChildItem C:\존재하지않는경로

# Step 4: Verify shell still works
Write-Output "한글 출력 테스트"
```

**Commands (Unix - Unicode test)**:

```bash
# Step 1: Create file with emoji
echo "🚀 Persistent Shell" > emoji_file.txt

# Step 2: Read emoji file
cat emoji_file.txt

# Step 3: Unicode in variable
export EMOJI_VAR="✅ Success"
echo $EMOJI_VAR
```

**Expected Results**:

- No crashes or encoding errors
- Korean/Unicode characters displayed (possibly with lossy conversion `�` on Windows)
- Shell continues to function after non-ASCII input/output

---

## 🧪 Advanced Test Scenarios

### Test 9: Long-running Command Timeout

**Objective**: Verify timeout handling works correctly.

**Commands**:

```bash
# Unix: Sleep for 2 seconds (should complete)
sleep 2
echo "Sleep completed"

# Unix: Sleep for 120 seconds with 60s timeout (should timeout)
# This should be tested with timeout parameter in tool call
```

**Windows**:

```powershell
# Sleep for 2 seconds (should complete)
Start-Sleep -Seconds 2
Write-Output "Sleep completed"

# Sleep for 120 seconds with 60s timeout (should timeout)
# This should be tested with timeout parameter in tool call
```

**Expected Results**:

- 2-second sleep completes successfully
- 120-second sleep with 60s timeout returns timeout error
- Shell remains usable after timeout

---

### Test 10: Interactive Command (Two-Tool Pattern)

**Objective**: Verify stdin input handling for interactive commands.

**Scenario**: Execute sudo command requiring password

**Step 1: Initial execution (should return UIResource)**:

```json
{
  "command": "sudo ls /root",
  "require_user_input": true
}
```

**Expected Response**:

```json
{
  "type": "ui_resource",
  "execution_id": "exec_abc123",
  "prompt": "Enter sudo password:"
}
```

**Step 2: Provide password**:

```json
{
  "execution_id": "exec_abc123",
  "user_input": "password123"
}
```

**Expected Result**:

- Password submitted via stdin (not visible in process list)
- Command executes with provided input
- Output shows directory listing or appropriate error

---

## 📊 Success Criteria

### Functional Requirements

- ✅ All working directory changes persist across commands
- ✅ Environment variables remain set across multiple commands
- ✅ Python/Node.js virtual environments stay activated
- ✅ Error commands don't crash the shell session
- ✅ Session isolation prevents cross-contamination
- ✅ UTF-8/Unicode handling doesn't cause crashes

### Performance Requirements

- ✅ Sequential commands execute 2-3x faster than one-shot mode
- ✅ Shell initialization overhead < 100ms
- ✅ No memory leaks during long-running sessions

### Security Requirements

- ✅ Interactive input (passwords) not exposed in process arguments
- ✅ Session cleanup occurs when session ends
- ✅ No zombie processes after shell termination

---

## 🚨 Known Limitations

1. **Interactive TUI Programs**: Tools like `vim`, `less`, `top` won't work (requires PTY)
   - **Workaround**: Use non-interactive alternatives (`cat`, `head`, `ps aux`)

2. **Multiple Password Prompts**: Commands requiring multiple inputs may fail
   - **Workaround**: Use credential caching or non-interactive authentication

3. **ANSI Escape Codes**: Output is plain text (no colors)
   - **Expected**: PowerShell `-NonInteractive` and bash output clean text

4. **Windows PowerShell Execution Policy**: May require `Set-ExecutionPolicy RemoteSigned`
   - **Setup**: Ensure execution policy allows script execution

---

## 📝 Test Execution Template

Use this template to record test results:

```markdown
### Test [Number]: [Name]

**Tester**: [Your Name/AI Agent ID]
**Date**: [Date]
**Platform**: [Windows 11 / Ubuntu 22.04 / macOS 14]
**Shell**: [PowerShell 5.1 / bash 5.2 / zsh 5.9]

**Commands Executed**:
[Paste exact commands]

**Actual Output**:
[Paste stdout/stderr/exit codes]

**Result**: ✅ PASS / ❌ FAIL / ⚠️ PARTIAL

**Notes**:
[Any observations, unexpected behavior, or issues]
```

---

## 🔄 Rollback Instructions

If critical issues are found:

1. **Disable Feature Flag**:

```json
{
  "command": "your_command",
  "use_persistent_shell": false
}
```

2. **Report Issue**: Document failing scenario with exact commands and output

3. **Fallback Mode**: System automatically uses one-shot execution when flag is `false`

---

## 📞 Support

For questions or issues during testing:

- **Documentation**: See `docs/architecture/persistent-shell-integration.md`
- **Code Reference**: `src-tauri/src/mcp/builtin/workspace/persistent_shell.rs`
- **POC Validation**: `stdio-repl-poc/src/main.rs`

---

**End of Test Request Document**

Please execute these test scenarios sequentially and report any failures or unexpected behavior. The persistent shell feature is designed to improve AI agent workflow efficiency by maintaining state across command executions.
