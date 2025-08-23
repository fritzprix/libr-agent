# Analysis of Interactive Browser Server Timeout Issue

## 1. Summary

The investigation into the `execute_script` timeout errors within the `InteractiveBrowserServer` (IBS) has revealed that the root cause is not a flaw in the script execution logic itself, but a critical issue in the application's state management. The `InteractiveBrowserServer` is being instantiated multiple times instead of operating as a singleton. This prevents the asynchronous callback mechanism from functioning correctly, as the instance handling the result from the browser is different from the instance that initiated the request, leading to an inevitable timeout.

## 2. Problem Description

Users of the Interactive Browser Server experience consistent 5-second timeouts when executing any command that relies on the `execute_script` function (e.g., `get_current_url`, `get_page_content`). The function call never returns the expected result from the browser session.

## 3. Log Analysis

A detailed analysis of the `ibs.txt` log file exposed a specific, repeating pattern that pointed to the root cause:

1.  **`IBS: execute_script_wrapper_executed`**: This log confirms that the JavaScript payload was successfully injected into the webview and began execution. This rules out issues with script evaluation itself.
2.  **`IBS: handle_script_result_no_waiter`**: This is the most critical log entry. It indicates that when the browser-side script finished and called the `browser_script_result` command, the corresponding `handle_script_result` function on the Rust backend could not find the pending "waiter" (a `oneshot::Sender`) required to send the result back to the original caller.
3.  **`IBS: execute_script_timeout`**: This error is logged exactly 5 seconds after the initial call. It is a direct consequence of the previous step. Since the waiter was not found, the result was never sent, and the waiting task in the original `execute_script` call timed out as expected.

This sequence demonstrates that the result is correctly coming back from the browser, but the backend is failing to connect it to the original request.

## 4. Code and Architecture Analysis

The code in `src-tauri/src/services/interactive_browser_server.rs` uses a standard asynchronous pattern:
- An `async` function (`execute_script`) places a `oneshot::Sender` in a shared `HashMap` (`result_waiters`) to act as a waiter.
- It then waits on the corresponding `oneshot::Receiver`.
- A separate function (`handle_script_result`), triggered by a Tauri command, is responsible for retrieving the waiter and sending the result.

This logic is sound *if and only if* both functions are operating on the **same instance** of `InteractiveBrowserServer` and therefore the same `result_waiters` map.

The architectural flaw is revealed by the startup logs:
```
[2025-08-22][11:48:49][...] IBS: new app_handle=...
[2025-08-22][11:48:49][...] IBS: new app_handle=...
```
The `InteractiveBrowserServer` is being constructed twice. This leads to a race condition that perfectly explains the logs:
- **Instance A** handles the `execute_script` call and stores the waiter in its internal map.
- The Tauri command `browser_script_result` is routed to **Instance B**.
- **Instance B** checks its own, empty map for the waiter, finds nothing, and logs `handle_script_result_no_waiter`.
- **Instance A** is left waiting for a result that was sent to the wrong place, causing the timeout.

## 5. Conclusion

The timeout issue is definitively caused by a state management misconfiguration within the Tauri application setup, which results in the `InteractiveBrowserServer` not being a true singleton. The fix requires refactoring the application's entry point (likely `src-tauri/src/main.rs`) to ensure a single, shared instance of the server is created and managed by Tauri's state system.
