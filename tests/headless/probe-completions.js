// Loads the demo, waits for "Ready", types code that should produce
// completions, triggers Monaco's suggest, and reads the suggestion DOM.
// This is the canonical CI smoke test.
import puppeteer from 'puppeteer';

const url = process.argv[2] || 'http://127.0.0.1:8090/';
const browser = await puppeteer.launch({ headless: 'new' });
const page = await browser.newPage();

const logs = [];
const errs = [];
page.on('console', (m) => logs.push(`[${m.type()}] ${m.text()}`));
page.on('pageerror', (e) => errs.push(`[pageerror] ${e.message}`));

await page.goto(url, { waitUntil: 'networkidle2', timeout: 60000 });

await page.waitForFunction(() => {
    const s = document.getElementById('ra-status');
    return !s || s.classList.contains('hidden') || /Ready/i.test(s.textContent);
}, { timeout: 60000 });
console.log('boot: ready');

const result = await page.evaluate(async () => {
    const monacoApi = window.editor;
    const editors = monacoApi.getEditors();
    if (!editors.length) return { error: 'no editor instance' };
    const ed = editors[0];
    const model = ed.getModel();

    model.setValue('fn main() {\n    let v: Vec<i32> = Vec::new();\n    v.\n}\n');
    ed.focus();
    ed.setPosition({ lineNumber: 3, column: 7 });

    ed.trigger('keyboard', 'editor.action.triggerSuggest', {});

    const t0 = Date.now();
    while (Date.now() - t0 < 8000) {
        const rows = document.querySelectorAll('.monaco-list-row');
        const labels = [...rows].map(r => {
            const lbl = r.querySelector('.label-name');
            return lbl ? lbl.textContent : null;
        }).filter(Boolean);
        if (labels.length) return { count: labels.length, sample: labels.slice(0, 15) };
        await new Promise(r => setTimeout(r, 200));
    }
    return { count: 0, sample: [], note: 'no suggest popup appeared' };
});

console.log('suggest result:', JSON.stringify(result, null, 2));

await browser.close();

if (errs.length) {
    console.log('\n--- pageerrors ---');
    console.log(errs.join('\n'));
}
const failures = logs.filter(l => /panicked|error\]/i.test(l));
if (failures.length) {
    console.log('\n--- console errors ---');
    console.log(failures.join('\n'));
}
process.exit(result.count > 0 && !errs.length ? 0 : 1);
