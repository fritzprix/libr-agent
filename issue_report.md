# `execute_windows_cmd` Tool Report

## 1. Description

This report details the `execute_windows_cmd` tool, its parameters, and typical responses based on observed behavior.

## 2. Parameters

The `execute_windows_cmd` tool accepts the following parameters:

- **`command`** (string, required): The command to execute using Windows PowerShell.
  - **Example**: `command = "python my_script.py"` or `command = "dir C:\Users"`

- **`input_prompt`** (string, optional): A custom message to display when user input is required.
  - **Example**: `input_prompt = "Please enter your password:"`

- **`input_type`** (enum, optional): Specifies the type of input expected ('password' for hidden input, 'text' for visible input).
  - **Accepted values**: `'password'`, `'text'`
  - **Example**: `input_type = "password"`

- **`require_user_input`** (boolean, optional): If `true`, the command will pause and prompt the user for input.
  - **Example**: `require_user_input = true`

- **`run_mode`** (enum, optional): Determines how the command is executed.
  - **Accepted values**: `'sync'`, `'async'`
  - **Default**: `'sync'`
  - **`'sync'`**: Waits for the command to complete and returns stdout/stderr immediately.
    - **Note**: Maximum timeout in sync mode is 30 seconds.
  - **`'async'`**: Runs the command in the background and immediately returns a `process_id`. The output can be retrieved later using `poll_process`.
    - **Note**: Recommended for commands expected to run longer than 30 seconds.

- **`timeout`** (float, optional): The maximum time in seconds to wait for the command to complete in `sync` mode.
  - **Default**: `30` seconds
  - **Note**: Only applicable in `sync` mode.

## 3. Responses

The `execute_windows_cmd` tool provides different responses based on the `run_mode` and execution outcome.

### 3.1. `sync` Mode Success Response

When `run_mode` is `'sync'` and the command completes successfully within the timeout, the response includes `stdout`, `stderr`, and `exit_code`.

- **Example Response:**
  ```json
  {
    "command": "echo Hello World",
    "exit_code": 0,
    "finished_at": "2025-11-22T05:15:00.000000000+00:00",
    "pid": 12345,
    "process_id": "some_unique_id",
    "started_at": "2025-11-22T05:14:59.000000000+00:00",
    "status": "finished",
    "stderr_size": 0,
    "stdout_size": 13,
    "streaming_available": true,
    "tail": {
      "lines": ["Hello World"],
      "source": "buffer",
      "src": "stdout"
    }
  }
  ```

### 3.2. `sync` Mode Timeout Error

If `run_mode` is `'sync'` and the command exceeds the specified `timeout` (default 30 seconds), an error is returned.

- **Example Error Response:**
  ```
  Error: Command execution timeout after 30 seconds (Code: -32603)
  For longer-running commands, set "run_mode" to "async" so the command runs in background and can be polled.
  You can adjust the default via the LIBRAGENT_DEFAULT_EXECUTION_TIMEOUT environment variable. (Code: -32603)
  ```

### 3.3. `async` Mode Success Response

When `run_mode` is `'async'`, the tool immediately returns a `process_id` and confirmation that the process has started in the background. The actual output and status must be retrieved using `poll_process`.

- **Example Response:**

  ```
  Process started in background.
  Process ID: uc0dhhdf926sd42px05gh8ej
  Command: python read_test_python.py

  Use 'poll_process' to check status and view output:
  poll_process(process_id: "uc0dhhdf926sd42px05gh8ej", tail: {src: "stdout", n: 20})

  Note: async mode is intended for long-running commands (over 30s).
  ```

### 3.4. `async` Mode `poll_process` Output

After an `async` command starts, `poll_process` is used to check its status and retrieve its output.

- **Example `poll_process` Response for a finished command:**
  ```json
  {
    "command": "python read_test_python.py",
    "exit_code": 0,
    "finished_at": "2025-11-22T05:09:41.212737300+00:00",
    "pid": 17300,
    "process_id": "uc0dhhdf926sd42px05gh8ej",
    "started_at": "2025-11-22T05:09:41.023582300+00:00",
    "status": "finished",
    "stderr_size": 0,
    "stdout_size": 28,
    "streaming_available": true,
    "tail": {
      "lines": ["This is a test from Python."],
      "source": "buffer",
      "src": "stdout"
    }
  }
  ```
