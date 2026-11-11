#!/usr/bin/env node
/**
 * libr-e2e-verify smoke matrix — one invocation, one pass/fail table.
 *
 * Runs: health -> create session -> message -> poll -> control-plane check
 *       -> transcript check -> terminate + delete.
 *
 * The control-plane step passes as "absent (yolo/unsafe fallback)" on release
 * builds (404) and only fails when an enabled plane reports an error.
 * Exit code 0 only when every step passes.
 *
 * Usage:
 *   node smoke_matrix.cjs --assistant <config-id> [--base-url http://localhost:3030]
 *     [--workspace <scratch-dir>] [--api-token <key>] [--execution-mode yolo|unsafe]
 *     [--data-dir <dir-with-e2e_control.cap>] [--timeout 180] [--keep-session]
 */
const fs = require('fs');
const path = require('path');

const args = process.argv.slice(2);
function arg(name, fallback) {
  const i = args.indexOf(`--${name}`);
  return i >= 0 && args[i + 1] !== undefined ? args[i + 1] : fallback;
}
function flag(name) {
  return args.includes(`--${name}`);
}

const baseUrl = (arg('base-url', 'http://localhost:3030')).replace(/\/$/, '');
const assistantId = arg('assistant', null);
const workspacePath = arg('workspace', null);
const apiToken = arg('api-token', process.env.LIBRAGENT_API_KEY || null);
const executionMode = arg('execution-mode', 'unsafe');
const dataDir = arg('data-dir', process.env.LIBRAGENT_DATA_DIR || null);
const timeoutSec = Number(arg('timeout', '180'));
const keepSession = flag('keep-session');

if (!assistantId) {
  console.error('Missing --assistant <config-id>. Run with --assistant <id>.');
  process.exit(2);
}

const results = [];
let sessionId = null;
let aborted = false;

function record(step, ok, evidence) {
  results.push({ step, ok, evidence });
  if (!ok) aborted = aborted || step === 'health' || step === 'create';
}

async function call(method, url, body, headers) {
  const h = Object.assign({ 'Content-Type': 'application/json' }, headers || {});
  if (apiToken) h['Authorization'] = `Bearer ${apiToken}`;
  const res = await fetch(url, {
    method,
    headers: h,
    body: body === undefined ? undefined : JSON.stringify(body),
    signal: AbortSignal.timeout(30000),
  });
  let json = null;
  try {
    json = await res.json();
  } catch (_) {
    /* non-JSON body */
  }
  return { status: res.status, json };
}

async function main() {
  // 1. Health gate
  try {
    const res = await call('GET', `${baseUrl}/api/health`);
    const ok = res.status === 200 && (!res.json || res.json.status === 'ok');
    record('health', ok, `GET /api/health -> ${res.status}`);
  } catch (err) {
    record('health', false, `GET /api/health failed: ${err.message}`);
  }
  if (aborted) return;

  // 2. Create an idle session (no request => no workflow started yet)
  const createBody = { assistantId, executionMode };
  if (workspacePath) createBody.workspacePath = workspacePath;
  try {
    const res = await call('POST', `${baseUrl}/api/sessions`, createBody);
    sessionId = res.json && res.json.id ? res.json.id : null;
    record(
      'create',
      res.status === 200 || res.status === 201 ? !!sessionId : false,
      `POST /api/sessions -> ${res.status} id=${sessionId || '-'}`,
    );
  } catch (err) {
    record('create', false, `POST /api/sessions failed: ${err.message}`);
  }
  if (!sessionId) return;

  try {
    // 3. Send one message
    const msg = await call('POST', `${baseUrl}/api/sessions/${sessionId}/messages`, {
      content: 'Reply with exactly: E2E-SMOKE-OK',
      source: 'api',
    });
    record(
      'message',
      msg.status === 200 && msg.json && (msg.json.status === 'processed' || msg.json.status === 'queued'),
      `POST messages -> ${msg.status} ${msg.json ? msg.json.status : '-'}`,
    );

    // 4. Poll until idle
    const deadline = Date.now() + timeoutSec * 1000;
    let status = '?';
    while (Date.now() < deadline) {
      const st = await call('GET', `${baseUrl}/api/sessions/${sessionId}`);
      status = st.json && st.json.status ? String(st.json.status).toLowerCase() : '?';
      if (status === 'idle') break;
      await new Promise((r) => setTimeout(r, 3000));
    }
    record('poll', status === 'idle', `status=${status} (timeout ${timeoutSec}s)`);

    // 5. Control plane (debug builds with LIBRAGENT_E2E_CONTROL=1)
    const capFile = dataDir ? path.join(dataDir, 'e2e_control.cap') : null;
    const cap = capFile && fs.existsSync(capFile) ? fs.readFileSync(capFile, 'utf8').trim() : null;
    const headers = cap ? { 'X-LibrAgent-E2E-Control': cap } : {};
    const st = await call('GET', `${baseUrl}/api/e2e-control/status`, undefined, headers);
    if (st.status === 404 || st.status === 405) {
      record('control-plane', true, `absent (${st.status}; yolo/unsafe fallback)`);
    } else if (st.status === 200) {
      const inter = await call('GET', `${baseUrl}/api/e2e-control/sessions/${sessionId}/interactions`, undefined, headers);
      const pend =
        inter.json && (inter.json.pendingApprovals || inter.json.pending_approvals) || [];
      let approved = 0;
      for (const p of pend) {
        const id = p.toolCallId || p.tool_call_id;
        if (!id) continue;
        const r = await call(
          'POST',
          `${baseUrl}/api/e2e-control/sessions/${sessionId}/tool-approvals`,
          { toolCallId: id, approved: true },
          headers,
        );
        if (r.status === 200) approved += 1;
      }
      record(
        'control-plane',
        inter.status === 200,
        `enabled; pending=${pend.length}; approved=${approved}`,
      );
    } else {
      record('control-plane', false, `GET /api/e2e-control/status -> ${st.status} (cap ${cap ? 'found' : 'missing'})`);
    }

    // 6. Transcript check (before cleanup)
    const msgs = await call('GET', `${baseUrl}/api/sessions/${sessionId}/messages?limit=500`);
    const list = msgs.json && (msgs.json.messages || msgs.json) || [];
    const assistantText = (Array.isArray(list) ? list : [])
      .filter((m) => m.role === 'assistant')
      .map((m) =>
        Array.isArray(m.content)
          ? m.content.filter((c) => c.type === 'text').map((c) => c.text || '').join('')
          : String(m.content || ''),
      )
      .join('\n');
    const excerpt = assistantText.trim().slice(0, 60).replace(/\s+/g, ' ');
    record(
      'transcript',
      msgs.status === 200 && assistantText.trim().length > 0,
      assistantText.includes('E2E-SMOKE-OK')
        ? 'assistant replied with E2E-SMOKE-OK'
        : `assistant reply: "${excerpt}"`,
    );
  } finally {
    // 7. Cleanup discipline
    if (!keepSession) {
      try {
        await call('POST', `${baseUrl}/api/sessions/${sessionId}/terminate`);
      } catch (_) {
        /* best effort */
      }
      const del = await call('DELETE', `${baseUrl}/api/sessions/${sessionId}`);
      record('cleanup', del.status === 200, `DELETE /api/sessions/${sessionId} -> ${del.status}`);
    } else {
      record('cleanup', true, `kept (--keep-session) id=${sessionId}`);
    }
  }
}

(async () => {
  try {
    await main();
  } catch (err) {
    record('abort', false, err.message);
  }
  const width = Math.max(6, ...results.map((r) => r.step.length));
  console.log(`\nlibr-e2e-verify smoke matrix — ${baseUrl} — session ${sessionId || '-'}\n`);
  for (const r of results) {
    console.log(`  ${r.step.padEnd(width)}  ${r.ok ? 'PASS' : 'FAIL'}  ${r.evidence}`);
  }
  const failed = results.filter((r) => !r.ok).length;
  console.log(`\n  ${results.length - failed}/${results.length} passed${failed ? ` — ${failed} failed` : ''}\n`);
  process.exit(failed === 0 ? 0 : 1);
})();
