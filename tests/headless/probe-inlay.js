// Boot the demo, wait for ready, then read the actual inlay hint label
// strings as Monaco computed them. Used to verify the doubled-colon fix
// (#56342d9) didn't regress.
import puppeteer from 'puppeteer';

const url = process.argv[2] || 'http://127.0.0.1:8090/';
const browser = await puppeteer.launch({ headless: 'new' });
const page = await browser.newPage();
await page.goto(url, { waitUntil: 'networkidle2', timeout: 60000 });
await page.waitForFunction(() => {
    const s = document.getElementById('ra-status');
    return !s || s.classList.contains('hidden') || /Ready/i.test(s.textContent);
}, { timeout: 90000 });

const hints = await page.evaluate(async () => {
    const ed = window.editor.getEditors()[0];
    ed.focus();
    await new Promise(r => setTimeout(r, 1500));
    const all = [...document.querySelectorAll('.view-overlays > div span, .view-line span')]
        .map(s => s.textContent)
        .filter(t => t && (t.includes(':') || t.startsWith('&')));
    return all.slice(0, 40);
});
console.log('hint-ish strings on page:');
console.log(hints);

await browser.close();
