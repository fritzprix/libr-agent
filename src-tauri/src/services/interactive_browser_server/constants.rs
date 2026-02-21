/// The initialization script injected into every browser window.
/// It sets up the `window.__LIBR_AGENT__` global object which handles:
/// 1. Waiting for Tauri IPC to be ready (critical for Linux/WebKitGTK)
/// 2. Executing scripts safely
/// 3. Sending results back to the Rust backend
pub const INIT_SCRIPT: &str = r#"
(function() {
    if (window.__LIBR_AGENT__) return;

    window.__LIBR_AGENT__ = {
        // Wait for Tauri IPC to be ready
        waitForIPC: async function(retries = 50, interval = 100) {
            if (window.__TAURI__) return true;
            console.log('[LibrAgent] Waiting for Tauri IPC...');
            for (let i = 0; i < retries; i++) {
                await new Promise(r => setTimeout(r, interval));
                if (window.__TAURI__) {
                    console.log('[LibrAgent] Tauri IPC is ready');
                    return true;
                }
            }
            console.error('[LibrAgent] Tauri IPC failed to initialize');
            return false;
        },

        // Send result back to Rust
        sendResult: async function(sessionId, requestId, result, isError = false) {
            if (!await this.waitForIPC()) {
                console.error('[LibrAgent] IPC not available, cannot send result');
                return;
            }

            const payload = {
                sessionId,
                requestId,
                result: isError ? `Error: ${result}` : (
                    typeof result === 'object' ? JSON.stringify(result) : String(result)
                )
            };

            try {
                await window.__TAURI__.core.invoke('browser_script_result', { payload });
            } catch (e) {
                console.error('[LibrAgent] Failed to invoke Tauri command:', e);
            }
        },

        // Execute user script safely
        execute: async function(sessionId, requestId, scriptContent) {
            console.log(`[LibrAgent] Executing request: ${requestId}`);
            try {
                // Use Function constructor to create an async function from the string
                const asyncFn = new Function('return (async () => { ' + scriptContent + ' })()');
                const result = await asyncFn();
                await this.sendResult(sessionId, requestId, result, false);
            } catch (e) {
                console.error('[LibrAgent] Script execution error:', e);
                await this.sendResult(sessionId, requestId, e.message, true);
            }
        }
    };

    console.log('[LibrAgent] Runtime initialized');
})();
"#;
