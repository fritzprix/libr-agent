// Early boot diagnostics for Windows release white screen debugging
console.log('[BOOT] LibrAgent initializing at', new Date().toISOString());
window.__BOOT_START = Date.now();

// Capture all errors during bootstrap phase
window.addEventListener('error', function (e) {
  console.error('[BOOT-ERROR]', {
    message: e.message,
    filename: e.filename,
    lineno: e.lineno,
    colno: e.colno,
    error: e.error ? e.error.stack : 'No stack trace',
  });
});

// Capture unhandled promise rejections
window.addEventListener('unhandledrejection', function (e) {
  console.error('[BOOT-PROMISE-REJECT]', {
    reason: e.reason,
    promise: e.promise,
  });
});

// Log when main script loads
console.log(
  '[BOOT] Event handlers registered, awaiting main script load',
);
