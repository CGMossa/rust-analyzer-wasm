// Loads a URL in headless chromium, prints console + page errors,
// and waits up to N seconds for a "Ready" status pill (or any panic).
// Usage: node probe.js <url> [timeoutSeconds]

import puppeteer from 'puppeteer';

const url = process.argv[2] || 'http://127.0.0.1:8090/';
const timeout = (Number(process.argv[3]) || 60) * 1000;

const browser = await puppeteer.launch({
    headless: 'new',
    args: ['--no-sandbox', '--disable-setuid-sandbox'],
});
const page = await browser.newPage();

const logs = [];
page.on('console', (msg) => {
    const text = msg.text();
    logs.push(`[${msg.type()}] ${text}`);
});
page.on('pageerror', (err) => {
    logs.push(`[pageerror] ${err.message}\n${err.stack || ''}`);
});
page.on('requestfailed', (req) => {
    logs.push(`[requestfailed] ${req.url()} :: ${req.failure() && req.failure().errorText}`);
});

const start = Date.now();
try {
    await page.goto(url, { waitUntil: 'networkidle2', timeout });
} catch (err) {
    logs.push(`[goto-error] ${err.message}`);
}

const result = await page.evaluate(async (deadline) => {
    return new Promise((resolve) => {
        const t0 = Date.now();
        const tick = () => {
            const status = document.getElementById('ra-status');
            const txt = status ? status.textContent : '';
            if (status && (status.classList.contains('hidden') || /Ready/i.test(txt))) {
                return resolve({ kind: 'ready', status: txt });
            }
            if (status && /error|fail/i.test(txt)) {
                return resolve({ kind: 'error', status: txt });
            }
            if (Date.now() - t0 > deadline) {
                return resolve({ kind: 'timeout', status: txt });
            }
            setTimeout(tick, 200);
        };
        tick();
    });
}, Math.max(1000, timeout - (Date.now() - start) - 500));

const elapsed = ((Date.now() - start) / 1000).toFixed(1);
console.log(`\n=== probe result (${elapsed}s) ===`);
console.log(`status: ${result.kind} :: ${result.status}`);
console.log(`\n=== console (${logs.length} lines) ===`);
console.log(logs.join('\n'));

await browser.close();
process.exit(result.kind === 'ready' ? 0 : 1);
