import * as monaco from 'monaco-editor/esm/vs/editor/edcore.main';
import exampleCode from './example-code.rs';

import './index.css';
import { conf, grammar } from './rust-grammar';
import fake_std_url from './fake_std.rs';
import fake_core_url from './fake_core.rs';
import fake_alloc_url from './fake_alloc.rs';

let state;
let allTokens;

self.MonacoEnvironment = {
    getWorker(_id, label) {
        if (label === 'editorWorkerService') {
            return new Worker(new URL('monaco-editor/esm/vs/editor/editor.worker.js', import.meta.url), { type: 'module' });
        }
        // No language-specific workers — Rust is wired through our wasm worker.
        return new Worker(new URL('monaco-editor/esm/vs/editor/editor.worker.js', import.meta.url), { type: 'module' });
    },
};

const modeId = 'rust';
monaco.languages.register({ id: modeId });

monaco.languages.onLanguage(modeId, () => {
    monaco.languages.setLanguageConfiguration(modeId, conf);
    monaco.languages.setMonarchTokensProvider(modeId, grammar);
});

const registerRA = () => {
    monaco.languages.registerHoverProvider(modeId, {
        provideHover: (_, pos) => state.hover(pos.lineNumber, pos.column),
    });

    monaco.languages.registerCodeLensProvider(modeId, {
        async provideCodeLenses(m) {
            const code_lenses = await state.code_lenses();
            const lenses = (code_lenses || []).map(({ range, command }) => {
                const position = { column: range.startColumn, lineNumber: range.startLineNumber };
                const references = command.positions.map((pos) => ({ range: pos, uri: m.uri }));
                return {
                    range,
                    command: {
                        id: command.id,
                        title: command.title,
                        arguments: [m.uri, position, references],
                    },
                };
            });
            return { lenses, dispose() { } };
        },
    });

    monaco.languages.registerReferenceProvider(modeId, {
        async provideReferences(m, pos, { includeDeclaration }) {
            const refs = await state.references(pos.lineNumber, pos.column, includeDeclaration);
            if (refs) return refs.map(({ range }) => ({ uri: m.uri, range }));
        },
    });

    monaco.languages.registerInlayHintsProvider(modeId, {
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
    });

    monaco.languages.registerDocumentHighlightProvider(modeId, {
        async provideDocumentHighlights(_, pos) {
            return await state.references(pos.lineNumber, pos.column, true);
        },
    });

    monaco.languages.registerRenameProvider(modeId, {
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
    });

    monaco.languages.registerCompletionItemProvider(modeId, {
        triggerCharacters: ['.', ':', '='],
        async provideCompletionItems(_m, pos) {
            const suggestions = await state.completions(pos.lineNumber, pos.column);
            if (suggestions) return { suggestions };
        },
    });

    monaco.languages.registerSignatureHelpProvider(modeId, {
        signatureHelpTriggerCharacters: ['(', ','],
        async provideSignatureHelp(_m, pos) {
            const value = await state.signature_help(pos.lineNumber, pos.column);
            if (!value) return null;
            return { value, dispose() { } };
        },
    });

    monaco.languages.registerDefinitionProvider(modeId, {
        async provideDefinition(m, pos) {
            const list = await state.definition(pos.lineNumber, pos.column);
            if (list) return list.map(def => ({ ...def, uri: m.uri }));
        },
    });

    monaco.languages.registerTypeDefinitionProvider(modeId, {
        async provideTypeDefinition(m, pos) {
            const list = await state.type_definition(pos.lineNumber, pos.column);
            if (list) return list.map(def => ({ ...def, uri: m.uri }));
        },
    });

    monaco.languages.registerImplementationProvider(modeId, {
        async provideImplementation(m, pos) {
            const list = await state.goto_implementation(pos.lineNumber, pos.column);
            if (list) return list.map(def => ({ ...def, uri: m.uri }));
        },
    });

    monaco.languages.registerDeclarationProvider(modeId, {
        async provideDeclaration(m, pos) {
            const list = await state.goto_declaration(pos.lineNumber, pos.column);
            if (list) return list.map(def => ({ ...def, uri: m.uri }));
        },
    });

    monaco.languages.registerDocumentSymbolProvider(modeId, {
        async provideDocumentSymbols() {
            return await state.document_symbols();
        },
    });

    monaco.languages.registerOnTypeFormattingEditProvider(modeId, {
        autoFormatTriggerCharacters: ['.', '='],
        async provideOnTypeFormattingEdits(_, pos, ch) {
            return await state.type_formatting(pos.lineNumber, pos.column, ch);
        },
    });

    monaco.languages.registerFoldingRangeProvider(modeId, {
        async provideFoldingRanges() {
            return await state.folding_ranges();
        },
    });

    monaco.languages.registerCodeActionProvider(modeId, {
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
    });
};

// RA worker proxy
const createRA = async () => {
    const worker = new Worker(new URL('./ra-worker.js', import.meta.url));
    const pendingResolve = {};
    let id = 1;
    let ready;

    const callWorker = (which, ...args) =>
        new Promise((resolve) => {
            pendingResolve[id] = resolve;
            worker.postMessage({ which, args, id });
            id += 1;
        });

    const proxyHandler = {
        get: (target, prop, receiver) => {
            if (prop === 'then') return Reflect.get(target, prop, receiver);
            return (...args) => callWorker(prop, ...args);
        },
    };

    worker.onmessage = (e) => {
        if (e.data.id === 'ra-worker-ready') {
            ready(new Proxy({}, proxyHandler));
            return;
        }
        const pending = pendingResolve[e.data.id];
        if (pending) {
            pending(e.data.result);
            delete pendingResolve[e.data.id];
        }
    };

    return new Promise((resolve) => { ready = resolve; });
};

const start = async () => {
    const loadingText = document.createTextNode('Loading wasm...');
    document.body.appendChild(loadingText);

    const model = monaco.editor.createModel(exampleCode, modeId);
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
            { token: 'variable', foreground: '9CDCFE' },
            { token: 'support.function', foreground: 'DCDCAA' },
        ],
    });

    document.body.removeChild(loadingText);

    const editor = monaco.editor.create(document.body, {
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
    });
    window.onresize = () => editor.layout();

    const initRA = async () => {
        state = await createRA();
        registerRA();
        const [fake_std, fake_core, fake_alloc] = await Promise.all([
            fetch(fake_std_url).then((r) => r.text()),
            fetch(fake_core_url).then((r) => r.text()),
            fetch(fake_alloc_url).then((r) => r.text()),
        ]);
        await state.init(model.getValue(), fake_std, fake_core, fake_alloc);
        await update();
        model.onDidChangeContent(update);
    };

    async function update() {
        const res = await state.update(model.getValue());
        monaco.editor.setModelMarkers(model, modeId, res.diagnostics);
        allTokens = res.highlights;
    }

    initRA();
};

start();
