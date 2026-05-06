import * as monaco from 'monaco-editor/esm/vs/editor/edcore.main';
import exampleCode from './example-code.rs';

import './index.css';
import { conf, grammar } from './rust-grammar';
import fake_std_url from './fake_std.rs';
import fake_core_url from './fake_core.rs';
import fake_alloc_url from './fake_alloc.rs';

let state;
let allTokens;
let editorModel;

self.MonacoEnvironment = {
    getWorker() {
        return new Worker(new URL('monaco-editor/esm/vs/editor/editor.worker.js', import.meta.url), { type: 'module' });
    },
};

const modeId = 'rust';
monaco.languages.register({ id: modeId });
monaco.languages.onLanguage(modeId, () => {
    monaco.languages.setLanguageConfiguration(modeId, conf);
    monaco.languages.setMonarchTokensProvider(modeId, grammar);
});

const FEATURES = [
    { id: 'completions',    label: 'Completions',           on: true },
    { id: 'hover',          label: 'Hover',                 on: true },
    { id: 'inlayHints',     label: 'Inlay hints',           on: true },
    { id: 'diagnostics',    label: 'Diagnostics',           on: true },
    { id: 'codeActions',    label: 'Code actions / fixes',  on: true },
    { id: 'signatureHelp',  label: 'Signature help',        on: true },
    { id: 'references',     label: 'References',            on: true },
    { id: 'definition',     label: 'Goto definition',       on: true },
    { id: 'declaration',    label: 'Goto declaration',      on: true },
    { id: 'implementation', label: 'Goto implementation',   on: true },
    { id: 'typeDefinition', label: 'Goto type definition',  on: true },
    { id: 'rename',         label: 'Rename',                on: true },
    { id: 'symbols',        label: 'Document symbols',      on: true },
    { id: 'codeLens',       label: 'Code lenses',           on: true },
    { id: 'folding',        label: 'Folding',               on: true },
    { id: 'highlight',      label: 'Document highlight',    on: true },
    { id: 'onTypeFormat',   label: 'On-type formatting',    on: true },
];

const loadSettings = () => {
    try {
        const saved = JSON.parse(localStorage.getItem('ra-settings') || '{}');
        const s = {};
        for (const f of FEATURES) s[f.id] = saved[f.id] ?? f.on;
        return s;
    } catch { return Object.fromEntries(FEATURES.map(f => [f.id, f.on])); }
};
const saveSettings = (s) => localStorage.setItem('ra-settings', JSON.stringify(s));
let settings = loadSettings();

const enabled = (id) => !!settings[id];

const setStatus = (msg) => {
    const el = document.getElementById('ra-status');
    if (!el) return;
    if (msg) { el.textContent = msg; el.classList.remove('hidden'); }
    else { el.classList.add('hidden'); }
};

// Worker proxy
const createRA = () => new Promise((resolve) => {
    const worker = new Worker(new URL('./ra-worker.js', import.meta.url));
    const pending = {};
    let id = 1;

    const call = (which, ...args) => new Promise((res, rej) => {
        const myId = id++;
        pending[myId] = { res, rej };
        worker.postMessage({ which, args, id: myId });
    });

    const proxy = new Proxy({}, {
        get: (_t, prop, _r) => (...args) => call(prop, ...args),
    });

    worker.onmessage = (e) => {
        if (e.data.id === 'ra-worker-ready') return resolve(proxy);
        const p = pending[e.data.id];
        if (!p) return;
        delete pending[e.data.id];
        if (e.data.error) p.rej(new Error(e.data.error));
        else p.res(e.data.result);
    };
});

// Wraps a provider so it's a no-op when settings disable it.
const gated = (id, provider) => {
    return new Proxy(provider, {
        get(target, prop) {
            const v = target[prop];
            if (typeof v !== 'function' || !prop.startsWith('provide')) return v;
            return (...args) => enabled(id) ? v.apply(target, args) : null;
        },
    });
};

const registerRA = () => {
    monaco.languages.registerHoverProvider(modeId, gated('hover', {
        provideHover: (_, pos) => state.hover(pos.lineNumber, pos.column),
    }));

    monaco.languages.registerCompletionItemProvider(modeId, gated('completions', {
        triggerCharacters: ['.', ':', '=', '>', '&', '(', ' '],
        async provideCompletionItems(_m, pos) {
            const suggestions = await state.completions(pos.lineNumber, pos.column);
            if (!suggestions) return { suggestions: [] };
            return { suggestions, incomplete: true };
        },
    }));

    monaco.languages.registerSignatureHelpProvider(modeId, gated('signatureHelp', {
        signatureHelpTriggerCharacters: ['(', ','],
        async provideSignatureHelp(_m, pos) {
            const value = await state.signature_help(pos.lineNumber, pos.column);
            if (!value) return null;
            return { value, dispose() { } };
        },
    }));

    monaco.languages.registerCodeLensProvider(modeId, gated('codeLens', {
        async provideCodeLenses(m) {
            const code_lenses = await state.code_lenses();
            const lenses = (code_lenses || []).map(({ range, command }) => ({
                range,
                command: {
                    id: command.id,
                    title: command.title,
                    arguments: [
                        m.uri,
                        { column: range.startColumn, lineNumber: range.startLineNumber },
                        command.positions.map((pos) => ({ range: pos, uri: m.uri })),
                    ],
                },
            }));
            return { lenses, dispose() { } };
        },
    }));

    monaco.languages.registerReferenceProvider(modeId, gated('references', {
        async provideReferences(m, pos, { includeDeclaration }) {
            const refs = await state.references(pos.lineNumber, pos.column, includeDeclaration);
            if (refs) return refs.map(({ range }) => ({ uri: m.uri, range }));
        },
    }));

    monaco.languages.registerInlayHintsProvider(modeId, gated('inlayHints', {
        async provideInlayHints(_model, _range, _token) {
            const hints = await state.inlay_hints();
            if (!hints) return { hints: [], dispose() { } };
            const out = [];
            for (const h of hints) {
                if (!h || !h.label) continue;
                const isParam = h.hint_type === 2;
                out.push({
                    kind: isParam
                        ? monaco.languages.InlayHintKind.Parameter
                        : monaco.languages.InlayHintKind.Type,
                    position: isParam
                        ? { lineNumber: h.range.startLineNumber, column: h.range.startColumn }
                        : { lineNumber: h.range.endLineNumber, column: h.range.endColumn },
                    label: isParam ? `${h.label}:` : `: ${h.label}`,
                    paddingLeft: !isParam,
                    paddingRight: isParam,
                });
            }
            return { hints: out, dispose() { } };
        },
    }));

    monaco.languages.registerDocumentHighlightProvider(modeId, gated('highlight', {
        async provideDocumentHighlights(_, pos) {
            return await state.references(pos.lineNumber, pos.column, true);
        },
    }));

    monaco.languages.registerRenameProvider(modeId, gated('rename', {
        async provideRenameEdits(m, pos, newName) {
            const edits = await state.rename(pos.lineNumber, pos.column, newName);
            if (edits) {
                return {
                    edits: edits.map(edit => ({ resource: m.uri, textEdit: edit, versionId: undefined })),
                };
            }
        },
        async resolveRenameLocation(_, pos) {
            return state.prepare_rename(pos.lineNumber, pos.column);
        },
    }));

    monaco.languages.registerDefinitionProvider(modeId, gated('definition', {
        async provideDefinition(m, pos) {
            const list = await state.definition(pos.lineNumber, pos.column);
            if (list) return list.map(def => ({ ...def, uri: m.uri }));
        },
    }));

    monaco.languages.registerTypeDefinitionProvider(modeId, gated('typeDefinition', {
        async provideTypeDefinition(m, pos) {
            const list = await state.type_definition(pos.lineNumber, pos.column);
            if (list) return list.map(def => ({ ...def, uri: m.uri }));
        },
    }));

    monaco.languages.registerImplementationProvider(modeId, gated('implementation', {
        async provideImplementation(m, pos) {
            const list = await state.goto_implementation(pos.lineNumber, pos.column);
            if (list) return list.map(def => ({ ...def, uri: m.uri }));
        },
    }));

    monaco.languages.registerDeclarationProvider(modeId, gated('declaration', {
        async provideDeclaration(m, pos) {
            const list = await state.goto_declaration(pos.lineNumber, pos.column);
            if (list) return list.map(def => ({ ...def, uri: m.uri }));
        },
    }));

    monaco.languages.registerDocumentSymbolProvider(modeId, gated('symbols', {
        async provideDocumentSymbols() {
            return await state.document_symbols();
        },
    }));

    monaco.languages.registerOnTypeFormattingEditProvider(modeId, gated('onTypeFormat', {
        autoFormatTriggerCharacters: ['.', '='],
        async provideOnTypeFormattingEdits(_, pos, ch) {
            return await state.type_formatting(pos.lineNumber, pos.column, ch);
        },
    }));

    monaco.languages.registerFoldingRangeProvider(modeId, gated('folding', {
        async provideFoldingRanges() {
            return await state.folding_ranges();
        },
    }));

    monaco.languages.registerCodeActionProvider(modeId, gated('codeActions', {
        async provideCodeActions(m, range, _context) {
            const assists = await state.assists(
                range.startLineNumber, range.startColumn,
                range.endLineNumber, range.endColumn,
            );
            if (!assists) return { actions: [], dispose() { } };
            const actions = assists.map(a => ({
                title: a.label,
                kind: a.kind === 'QuickFix' ? 'quickfix' : 'refactor',
                isPreferred: a.kind === 'QuickFix',
                edit: a.edits.length === 0 ? undefined : {
                    edits: a.edits.map(e => ({ resource: m.uri, textEdit: e, versionId: undefined })),
                },
            }));
            return { actions, dispose() { } };
        },
    }));
};

const buildSettingsPanel = (model) => {
    const root = document.createElement('div');
    root.id = 'ra-settings';
    root.innerHTML = `<h3>Language features</h3>` + FEATURES.map(f =>
        `<label><input type="checkbox" data-id="${f.id}" ${enabled(f.id) ? 'checked' : ''}> ${f.label}</label>`
    ).join('');
    document.body.appendChild(root);

    const toggle = document.createElement('button');
    toggle.id = 'ra-settings-toggle';
    toggle.textContent = '⚙ Settings';
    document.body.appendChild(toggle);

    toggle.addEventListener('click', () => root.classList.toggle('open'));
    document.addEventListener('click', (e) => {
        if (!root.contains(e.target) && e.target !== toggle) root.classList.remove('open');
    });

    root.addEventListener('change', (e) => {
        const cb = e.target;
        if (cb.tagName !== 'INPUT') return;
        settings[cb.dataset.id] = cb.checked;
        saveSettings(settings);
        // Force Monaco to re-query providers (e.g. for diagnostics, inlayHints).
        if (cb.dataset.id === 'diagnostics' && !cb.checked) {
            monaco.editor.setModelMarkers(model, modeId, []);
        } else if (cb.dataset.id === 'diagnostics' && cb.checked) {
            update();
        }
    });
};

const start = async () => {
    const model = monaco.editor.createModel(exampleCode, modeId);
    editorModel = model;
    window.editor = monaco.editor;

    monaco.editor.defineTheme('vscode-dark-plus', {
        base: 'vs-dark',
        inherit: true,
        colors: {
            'editorInlayHint.foreground': '#A0A0A0F0',
            'editorInlayHint.background': '#11223300',
        },
        rules: [
            { token: 'keyword.control', foreground: 'C586C0' },
            { token: 'variable',        foreground: '9CDCFE' },
            { token: 'support.function', foreground: 'DCDCAA' },
        ],
    });

    const host = document.createElement('div');
    host.id = 'editor';
    document.body.appendChild(host);

    const status = document.createElement('div');
    status.id = 'ra-status';
    status.textContent = 'Loading rust-analyzer (wasm)…';
    document.body.appendChild(status);

    buildSettingsPanel(model);

    const editor = monaco.editor.create(host, {
        theme: 'vscode-dark-plus',
        model,
        automaticLayout: true,
        inlayHints: { enabled: 'on' },
        bracketPairColorization: { enabled: true },
        guides: { bracketPairs: true, indentation: true },
        renderWhitespace: 'selection',
        smoothScrolling: true,
        cursorBlinking: 'smooth',
        cursorSmoothCaretAnimation: 'on',
        formatOnType: true,
        suggest: { showStatusBar: true, preview: true, previewMode: 'subwordSmart' },
        fontLigatures: true,
        minimap: { enabled: true, renderCharacters: false },
        lightbulb: { enabled: 'on' },
        quickSuggestions: { other: true, comments: false, strings: false },
        suggestOnTriggerCharacters: true,
    });
    window.onresize = () => editor.layout();

    setStatus('Spawning analyzer worker…');
    state = await createRA();

    setStatus('Loading sysroot…');
    const [fake_std, fake_core, fake_alloc] = await Promise.all([
        fetch(fake_std_url).then((r) => r.text()),
        fetch(fake_core_url).then((r) => r.text()),
        fetch(fake_alloc_url).then((r) => r.text()),
    ]);

    setStatus('Initializing analyzer…');
    await state.init(model.getValue(), fake_std, fake_core, fake_alloc);

    // Register providers AFTER init — otherwise the analyzer has an empty
    // crate graph and Monaco caches the resulting empty results.
    registerRA();
    await update();
    model.onDidChangeContent(() => { if (enabled('diagnostics')) update(); });

    setStatus('Ready');
    setTimeout(() => setStatus(null), 1200);
};

async function update() {
    const res = await state.update(editorModel.getValue());
    monaco.editor.setModelMarkers(editorModel, modeId, res.diagnostics || []);
    allTokens = res.highlights;
}

start();
