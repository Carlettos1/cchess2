import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import { fileURLToPath } from 'node:url';
import type { Plugin } from 'vite';

// Local development only. Production uses the same session through Tauri IPC.
export function enginePlugin(): Plugin {
  return { name: 'cchess-engine', configureServer(server) {
    if (process.env.VITEST) return;
    const child = spawn('cargo', ['run', '--quiet', '--manifest-path', fileURLToPath(new URL('../session/Cargo.toml', import.meta.url)), '--bin', 'cchess-bridge'], { stdio: ['pipe', 'pipe', 'inherit'] });
    let sequence = 0;
    let failed = false;
    const pending = new Map<number, { resolve: (value: unknown) => void; reject: (error: Error) => void }>();
    const fail = (error: Error) => { failed = true; pending.forEach(p => p.reject(error)); pending.clear(); };
    child.on('error', fail);
    child.on('exit', () => fail(new Error('Engine stopped. Restart the development server.')));
    child.stdin.on('error', fail);
    createInterface({ input: child.stdout }).on('line', line => {
      try { const result = JSON.parse(line); const waiter = pending.get(result.id); pending.delete(result.id); if (result.error) waiter?.reject(new Error(result.error)); else waiter?.resolve(result.value); }
      catch { fail(new Error('Invalid engine response')); }
    });
    server.httpServer?.once('close', () => { child.kill(); fail(new Error('Server closed')); });
    server.middlewares.use('/api/game', (req, res) => {
      if (req.headers.origin && req.headers.origin !== `http://${req.headers.host}` && req.headers.origin !== `https://${req.headers.host}`) { res.statusCode = 403; res.end('Origin not allowed'); return; }
      if (req.method !== 'POST') { res.statusCode = 405; res.end('POST required'); return; }
      let body = ''; let tooLarge = false;
      req.on('data', chunk => { body += chunk; if (body.length > 8_000_000) { tooLarge = true; res.statusCode = 413; res.end('Save is too large'); req.destroy(); } });
      req.on('end', async () => {
        if (tooLarge) return;
        try {
          if (failed) throw new Error('Engine unavailable');
          const request = JSON.parse(body); const id = ++sequence;
          const value = await new Promise((resolve, reject) => { pending.set(id, { resolve, reject }); child.stdin.write(JSON.stringify({ id, request }) + '\n'); });
          res.setHeader('Content-Type', 'application/json'); res.end(JSON.stringify(value));
        } catch (error) { res.statusCode = 400; res.end(error instanceof Error ? error.message : String(error)); }
      });
    });
  } };
}
