// Verbose probe: captures all logs/errors regardless of timeout. Useful
// when probe.js times out and you want to see what the page logged.
import puppeteer from 'puppeteer';

const url = process.argv[2] || 'http://127.0.0.1:8090/';
const maxWait = (Number(process.argv[3]) || 90) * 1000;

const browser = await puppeteer.launch({
    headless: 'new',
    args: ['--no-sandbox', '--disable-setuid-sandbox'],
});
const page = await browser.newPage();

const logs = [];
const errs = [];
page.on('console', (m) => logs.push(`[${m.type()}] ${m.text()}`));
page.on('pageerror', (e) => errs.push(`[pageerror] ${e.message}\n${(e.stack || '').slice(0, 800)}`));
page.on('requestfailed', (r) => errs.push(`[requestfailed] ${r.url()} :: ${r.failure() && r.failure().errorText}`));

console.log(`loading ${url} ...`);
try {
    await page.goto(url, { waitUntil: 'networkidle2', timeout: maxWait });
} catch (e) {
    console.log(`goto: ${e.message}`);
}

const t0 = Date.now();
let ready = false;
while (Date.now() - t0 < maxWait) {
    const status = await page.evaluate(() => {
        const s = document.getElementById('ra-status');
        if (!s) return { exists: false };
        return { exists: true, text: s.textContent, hidden: s.classList.contains('hidden') };
    }).catch(() => ({ error: true }));
    if (status.error) break;
    if (!status.exists || status.hidden || /Ready/i.test(status.text)) {
        ready = true;
        console.log(`status pill: ${JSON.stringify(status)} (after ${(Date.now()-t0)/1000}s)`);
        break;
    }
    if (Date.now() - t0 < 5000 || Math.floor((Date.now() - t0) / 5000) > Math.floor((Date.now() - t0 - 1000) / 5000)) {
        console.log(`  status pill: ${status.text} (${((Date.now()-t0)/1000).toFixed(0)}s)`);
    }
    await new Promise(r => setTimeout(r, 1000));
}
if (!ready) console.log(`NOT READY after ${maxWait/1000}s`);

await browser.close();
console.log(`\n--- console (${logs.length} lines) ---`);
console.log(logs.join('\n'));
if (errs.length) {
    console.log(`\n--- errors (${errs.length}) ---`);
    console.log(errs.join('\n'));
}
process.exit(ready ? 0 : 1);
