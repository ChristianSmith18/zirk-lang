const vscode = require('vscode');
const { spawn } = require('node:child_process');
const path = require('node:path');
const fs = require('node:fs');

const LANGUAGE_ID = 'zirk';
const DIAGNOSTIC_SOURCE = 'zirk';
const OUTPUT_CHANNEL_NAME = 'Zirk';
const CONFIG_SECTION = 'zirk';

let diagnosticCollection;
let outputChannel;
let statusBarItem;
let tripleQuoteGuide;
let tripleQuoteEndpoint;

// Set while a `zirk-lsp` language client is running. The server then owns
// diagnostics, hover and formatting for `zirk` documents, and the CLI bridge
// code paths below stay silent (spec `zirk-lsp`, "Editor integration with
// CLI fallback").
let languageClient;

const pendingLiveChecks = new Map();
const runningLiveChecks = new Set();
const liveTempFiles = new Set();

// `zirk check --types` output, per document URI — refreshed by the same
// bridge pass that produces diagnostics, so hover never spawns the
// compiler on its own (`editor-cli-bridge`, "Type-map retrieval").
const typeMaps = new Map();

// Brand syntax palette (zirk-lang-site tokens). Every scope ends in `.zirk`,
// so the rules only ever touch Zirk files — the active theme stays in charge
// of everything else. Applied/removed at runtime by `applyBrandPalette`.
const BRAND_TEXTMATE_RULES = [
    // keywords declarativas: class, fn, import, trait…
    { scope: ['keyword.declaration.zirk', 'keyword.control.import.zirk', 'storage.type.function.zirk'], settings: { foreground: '#8E6CFF' } },
    // control de flujo: if, match, try, for, task…
    { scope: 'keyword.control.zirk', settings: { foreground: '#EC4899' } },
    // salidas: return, throw, break, continue
    { scope: 'keyword.control.exit.zirk', settings: { foreground: '#F7C65C' } },
    // mutabilidad: mut, inmut, inmut::strict, share, sync
    { scope: 'storage.modifier.mutability.zirk', settings: { foreground: '#F7C65C' } },
    // modificadores: public, private, static, final, inner, abstract…
    { scope: 'storage.modifier.zirk', settings: { foreground: '#A78BFA' } },
    // tipos builtin: Int32, String, Void…
    { scope: 'storage.type.zirk', settings: { foreground: '#32D4C6' } },
    // clases/contratos de usuario (Capitalized)
    { scope: ['entity.name.type.zirk', 'entity.name.class.zirk'], settings: { foreground: '#34D399' } },
    // tipos importados y alias de tipo
    { scope: ['entity.name.type.imported.zirk', 'entity.name.type.import.alias.zirk'], settings: { foreground: '#34D399' } },
    // declaración de fn/método
    { scope: 'entity.name.function.zirk', settings: { foreground: '#39C6FF' } },
    // llamadas
    { scope: 'entity.name.function.call.zirk', settings: { foreground: '#38BDF8' } },
    // strings, chars, regex
    { scope: ['string.quoted.double.zirk', 'string.quoted.triple.zirk', 'string.regexp.zirk', 'constant.character.zirk'], settings: { foreground: '#FF4FA3' } },
    // números y duraciones
    { scope: ['constant.numeric.integer.zirk', 'constant.numeric.float.zirk', 'constant.numeric.hex.zirk', 'constant.numeric.octal.zirk', 'constant.numeric.binary.zirk', 'constant.numeric.duration.zirk'], settings: { foreground: '#60A5FA' } },
    // constantes booleanas y opcionales
    { scope: 'constant.language.zirk', settings: { foreground: '#FF6B81' } },
    // variantes Result: éxito en verde, error en coral
    { scope: 'constant.language.result-success.zirk', settings: { foreground: '#34D399' } },
    { scope: 'constant.language.result-error.zirk', settings: { foreground: '#FF6B81' } },
    // this / super / outer
    { scope: 'variable.language.zirk', settings: { foreground: '#FFD166' } },
    // decoradores @name
    { scope: ['meta.decorator.zirk', 'entity.name.function.decorator.zirk', 'punctuation.decorator.zirk'], settings: { foreground: '#F4A261' } },
    // marcadores #name (#override)
    { scope: 'comment.line.marker.zirk', settings: { foreground: '#6A6A75' } },
    // módulos en imports: std.io y similares
    { scope: 'support.module.standard.zirk', settings: { foreground: '#F4A261' } },
    // rutas de paquetes externos como texto neutral
    { scope: 'support.module.external.zirk', settings: { foreground: '#D4D4D8' } },
    // separador de submódulo estándar
    { scope: 'punctuation.separator.module-path.zirk', settings: { foreground: '#F7F7FA' } },
    // funciones y valores importados y alias de valor
    { scope: ['variable.other.imported.zirk', 'variable.other.import.alias.zirk'], settings: { foreground: '#39C6FF' } },
    // nombres importados sin alias (otros contextos)
    { scope: 'variable.other.readwrite.zirk', settings: { foreground: '#F7F7FA' } },
    // builtins: stdin/stdout/stderr, println, assert…
    { scope: ['support.variable.zirk', 'support.function.zirk'], settings: { foreground: '#39C6FF' } },
    // argumentos nombrados
    { scope: 'variable.parameter.zirk', settings: { foreground: '#B8B8C3', fontStyle: 'italic' } },
    // operadores (incluye `->` de alias, `is`/`as?` de cast, `|>` pipe)
    { scope: ['keyword.operator.zirk', 'keyword.operator.alias.zirk', 'keyword.operator.cast.zirk', 'keyword.operator.pipe.zirk', 'keyword.operator.range.zirk'], settings: { foreground: '#A78BFA' } },
    { scope: ['punctuation.section.array.begin.zirk', 'punctuation.section.array.end.zirk', 'punctuation.section.brackets.begin.zirk', 'punctuation.section.brackets.end.zirk'], settings: { foreground: '#8E6CFF' } },
    // puntuación de interpolación `{`/`}` dentro de strings
    { scope: ['punctuation.section.embedded.begin.zirk', 'punctuation.section.embedded.end.zirk', 'punctuation.definition.placeholder.zirk', 'punctuation.separator.placeholder.zirk'], settings: { foreground: '#8E6CFF' } },
    // comentarios normales (// y /* */)
    { scope: ['comment.line.double-slash.zirk', 'comment.block.zirk'], settings: { foreground: '#A3A3AF', fontStyle: 'italic' } },
];

/** True when a textMateRule's scope belongs to Zirk (ours or the user's own). */
function isZirkScopeRule(rule) {
    const scopes = Array.isArray(rule.scope) ? rule.scope : [rule.scope];
    return scopes.some(s => typeof s === 'string' && s.includes('.zirk'));
}

/**
 * Adds or removes the brand rules in `editor.tokenColorCustomizations`,
 * preserving any user-defined rules. Scopes are `.zirk`-suffixed, so other
 * languages are untouched even though the setting is global.
 */
async function applyBrandPalette(enabled) {
    const editorConfig = vscode.workspace.getConfiguration('editor');
    const current = editorConfig.get('tokenColorCustomizations') || {};
    const kept = (current.textMateRules || []).filter(rule => !isZirkScopeRule(rule));
    const textMateRules = enabled ? [...kept, ...BRAND_TEXTMATE_RULES] : kept;
    const next = { ...current, textMateRules };
    if (next.textMateRules.length === 0) {
        delete next.textMateRules;
    }
    try {
        await editorConfig.update('tokenColorCustomizations', next, vscode.ConfigurationTarget.Global);
    } catch (err) {
        outputChannel?.appendLine(`[palette] could not update tokenColorCustomizations: ${err}`);
    }
}

const DEPRECATED_TYPES = new Map([
    ['BinaryFloat', 'Float'],
    ['BinaryFloat16', 'Float16'],
    ['BinaryFloat32', 'Float32'],
    ['BinaryFloat64', 'Float64'],
    ['BinaryFloat128', 'Float128'],
    ['Decimal16', 'Decimal'],
    ['Decimal32', 'Decimal'],
    ['Decimal64', 'Decimal'],
    ['Decimal128', 'Decimal'],
]);

async function activate(context) {
    outputChannel = vscode.window.createOutputChannel(OUTPUT_CHANNEL_NAME);
    diagnosticCollection = vscode.languages.createDiagnosticCollection(DIAGNOSTIC_SOURCE);

    context.subscriptions.push(outputChannel, diagnosticCollection);

    // The language server takes over diagnostics, hover and formatting when
    // its binary exists; everything else below is untouched either way.
    const serverRunning = await startLanguageClient(context);
    if (serverRunning) {
        // The bridge may already have run once while the client started —
        // its entries would sit next to the server's own diagnostics.
        diagnosticCollection.clear();
    }

    tripleQuoteGuide = vscode.window.createTextEditorDecorationType({
        // Each content row gets the same `│` cap as the delimiters, anchored
        // at the opening delimiter column. The thin space keeps content clear
        // of the guide without changing the source or its indentation.
        before: {
            contentText: '│\u2009',
            color: '#FF4FA3',
        },
        rangeBehavior: vscode.DecorationRangeBehavior.ClosedClosed,
    });
    // The first quote becomes a cap for the ruler (`│''`) only when the
    // scanner found a complete `''' … '''` pair. The source stays unchanged.
    tripleQuoteEndpoint = vscode.window.createTextEditorDecorationType({
        color: 'transparent',
        before: {
            contentText: '│',
            color: '#FF4FA3',
            margin: '0 -1ch 0 0',
        },
        rangeBehavior: vscode.DecorationRangeBehavior.ClosedClosed,
    });
    context.subscriptions.push(tripleQuoteGuide, tripleQuoteEndpoint);

    statusBarItem = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 0);
    statusBarItem.text = serverRunning ? '$(server) Zirk: LSP' : 'Zirk';
    statusBarItem.tooltip = serverRunning
        ? 'The Zirk language server is handling this file'
        : 'Click to re-check the active Zirk file';
    statusBarItem.command = serverRunning ? undefined : 'zirk.check';
    statusBarItem.show();
    context.subscriptions.push(statusBarItem);

    const checkCommand = vscode.commands.registerCommand('zirk.check', () => {
        const editor = vscode.window.activeTextEditor;
        if (editor?.document.languageId === LANGUAGE_ID) {
            checkDocument(editor.document, true);
        }
    });

    const buildCommand = vscode.commands.registerCommand('zirk.build', () => {
        const editor = vscode.window.activeTextEditor;
        if (editor?.document.languageId === LANGUAGE_ID) {
            runBuildOrRun(editor.document, 'build');
        }
    });

    const runCommand = vscode.commands.registerCommand('zirk.run', () => {
        const editor = vscode.window.activeTextEditor;
        if (editor?.document.languageId === LANGUAGE_ID) {
            runBuildOrRun(editor.document, 'run');
        }
    });

    context.subscriptions.push(checkCommand, buildCommand, runCommand);

    const openDisposable = vscode.workspace.onDidOpenTextDocument(document => {
        if (document.languageId === LANGUAGE_ID) {
            checkDocument(document);
        }
    });

    const saveDisposable = vscode.workspace.onDidSaveTextDocument(document => {
        if (document.languageId === LANGUAGE_ID) {
            checkDocument(document);
        }
    });

    const changeDisposable = vscode.workspace.onDidChangeTextDocument(event => {
        const document = event.document;
        updateTripleQuoteGuideForDocument(document);
        if (document.languageId !== LANGUAGE_ID || document.isUntitled || document.uri.scheme !== 'file') {
            return;
        }
        const config = vscode.workspace.getConfiguration(CONFIG_SECTION, document.uri);
        if (config.get('checkOnType', true)) {
            scheduleLiveCheck(document);
        }
    });

    const closeDisposable = vscode.workspace.onDidCloseTextDocument(document => {
        const key = document.uri.toString();
        const timer = pendingLiveChecks.get(key);
        if (timer) {
            clearTimeout(timer);
            pendingLiveChecks.delete(key);
        }
        runningLiveChecks.delete(key);
        removeLiveTempFile(document.uri.fsPath);
        diagnosticCollection.delete(document.uri);
        typeMaps.delete(document.uri.toString());
    });

    const activeEditorDisposable = vscode.window.onDidChangeActiveTextEditor(editor => {
        if (editor?.document.languageId === LANGUAGE_ID) {
            updateStatusBarForDocument(editor.document);
        }
        updateTripleQuoteGuide(editor);
    });

    context.subscriptions.push(openDisposable, saveDisposable, changeDisposable, closeDisposable, activeEditorDisposable);

    applyBrandPalette(vscode.workspace.getConfiguration(CONFIG_SECTION).get('brandColors', true));
    const configDisposable = vscode.workspace.onDidChangeConfiguration(event => {
        if (event.affectsConfiguration(`${CONFIG_SECTION}.brandColors`)) {
            applyBrandPalette(vscode.workspace.getConfiguration(CONFIG_SECTION).get('brandColors', true));
        }
    });
    context.subscriptions.push(configDisposable);

    for (const document of vscode.workspace.textDocuments) {
        if (document.languageId === LANGUAGE_ID) {
            checkDocument(document);
        }
    }
    updateTripleQuoteGuide(vscode.window.activeTextEditor);

    const taskProvider = vscode.tasks.registerTaskProvider('zirk', {
        provideTasks() {
            return provideZirkTasks();
        },
        resolveTask(task) {
            return resolveZirkTask(task);
        }
    });
    context.subscriptions.push(taskProvider);

    const codeActions = vscode.languages.registerCodeActionsProvider(LANGUAGE_ID, {
        provideCodeActions(document, range, context) {
            return provideDeprecatedTypeActions(document, context);
        }
    }, {
        providedCodeActionKinds: [vscode.CodeActionKind.QuickFix]
    });
    context.subscriptions.push(codeActions);

    if (!serverRunning) {
        const formatter = vscode.languages.registerDocumentFormattingEditProvider(LANGUAGE_ID, {
            async provideDocumentFormattingEdits(document, options, token) {
                const formatted = await tryZirkFormat(document);
                if (formatted !== undefined) {
                    const lastLine = document.lineAt(document.lineCount - 1);
                    const fullRange = new vscode.Range(0, 0, document.lineCount - 1, lastLine.text.length);
                    return [vscode.TextEdit.replace(fullRange, formatted)];
                }
                return provisionalFormat(document, options);
            }
        });

        context.subscriptions.push(formatter);

        const hover = vscode.languages.registerHoverProvider(LANGUAGE_ID, {
            provideHover(document, position) {
                return provideTypeHover(document, position);
            }
        });
        context.subscriptions.push(hover);
    }
}

/** Paints the opening delimiter column strictly between each `'''` pair. */
function updateTripleQuoteGuide(editor) {
    if (!tripleQuoteGuide || !editor || editor.document.languageId !== LANGUAGE_ID) {
        return;
    }
    const delimiters = [];
    const matcher = /'''/g;
    const text = editor.document.getText();
    let match;
    while ((match = matcher.exec(text)) !== null) {
        delimiters.push(match.index);
    }

    const ranges = [];
    const endpoints = [];
    for (let i = 0; i + 1 < delimiters.length; i += 2) {
        const open = editor.document.positionAt(delimiters[i]);
        const close = editor.document.positionAt(delimiters[i + 1]);
        endpoints.push(
            new vscode.Range(open, open.translate(0, 1)),
            new vscode.Range(close, close.translate(0, 1))
        );
        for (let line = open.line + 1; line < close.line; line += 1) {
            const character = Math.min(open.character, editor.document.lineAt(line).text.length);
            // Place the virtual cap at the same source column as the opening
            // delimiter. The line's first content character remains editable.
            const position = new vscode.Position(line, character);
            ranges.push(new vscode.Range(position, position));
        }
    }
    editor.setDecorations(tripleQuoteGuide, ranges);
    editor.setDecorations(tripleQuoteEndpoint, endpoints);
}

function updateTripleQuoteGuideForDocument(document) {
    const editor = vscode.window.activeTextEditor;
    if (editor?.document === document) {
        updateTripleQuoteGuide(editor);
    }
}

function provisionalFormat(document, options) {
    const textEdits = [];
    const lineCount = document.lineCount;
    let indentLevel = 0;
    const tabSize = options.tabSize || 4;
    const insertSpaces = options.insertSpaces !== false;
    const indentStr = insertSpaces ? ' '.repeat(tabSize) : '\t';

    for (let i = 0; i < lineCount; i++) {
        const line = document.lineAt(i);
        let text = line.text.trim();

        if (text.startsWith('}') || text.startsWith(']')) {
            indentLevel = Math.max(0, indentLevel - 1);
        }

        let formatted = text;
        if (text.length > 0) {
            formatted = indentStr.repeat(indentLevel) + text;
        }

        if (text.endsWith('{') || text.endsWith('[')) {
            indentLevel++;
        }

        if (formatted !== line.text) {
            const range = new vscode.Range(i, 0, i, line.text.length);
            textEdits.push(vscode.TextEdit.replace(range, formatted));
        }
    }
    return textEdits;
}

function tryZirkFormat(document) {
    const executable = findCompiler(document.uri, 'build');
    if (!executable) {
        return Promise.resolve(undefined);
    }

    const filePath = document.uri.fsPath;
    const tempPath = path.join(path.dirname(filePath), `.${path.basename(filePath)}.zirk-fmt.zrk`);
    const before = document.getText();

    try {
        fs.writeFileSync(tempPath, before);
    } catch {
        return Promise.resolve(undefined);
    }

    return new Promise(resolve => {
        const child = spawn(executable, ['format', tempPath], {
            cwd: path.dirname(tempPath),
        });

        let stdout = '';
        child.stdout.on('data', chunk => {
            stdout += chunk.toString();
        });

        const finish = result => {
            try {
                fs.unlinkSync(tempPath);
            } catch {
                // temp file may already be gone
            }
            resolve(result);
        };

        child.on('error', () => finish(undefined));
        child.on('close', code => {
            if (code !== 0) {
                finish(undefined);
                return;
            }

            let afterFile;
            try {
                afterFile = fs.readFileSync(tempPath, 'utf8');
            } catch {
                afterFile = undefined;
            }

            if (afterFile !== undefined && afterFile !== before) {
                finish(afterFile);
            } else if (stdout.trim().length > 0) {
                finish(stdout);
            } else {
                finish(undefined);
            }
        });
    });
}

function provideDeprecatedTypeActions(document, context) {
    const actions = [];
    for (const diagnostic of context.diagnostics) {
        if (diagnostic.source !== DIAGNOSTIC_SOURCE) {
            continue;
        }

        const wordRange = document.getWordRangeAtPosition(
            diagnostic.range.start,
            /[A-Za-z_]\w*/
        );
        if (!wordRange) {
            continue;
        }

        const name = document.getText(wordRange);
        const replacement = DEPRECATED_TYPES.get(name);
        if (!replacement) {
            continue;
        }

        const action = new vscode.CodeAction(
            `Replace \`${name}\` with \`${replacement}\``,
            vscode.CodeActionKind.QuickFix
        );
        action.diagnostics = [diagnostic];
        action.isPreferred = true;
        action.edit = new vscode.WorkspaceEdit();
        action.edit.replace(document.uri, wordRange, replacement);
        actions.push(action);
    }
    return actions;
}

function deactivate() {
    for (const timer of pendingLiveChecks.values()) {
        clearTimeout(timer);
    }
    pendingLiveChecks.clear();
    runningLiveChecks.clear();
    typeMaps.clear();
    for (const tempPath of liveTempFiles) {
        try {
            fs.unlinkSync(tempPath);
        } catch {
            // already gone
        }
    }
    liveTempFiles.clear();
}

function liveTempPath(filePath) {
    return path.join(path.dirname(filePath), `.${path.basename(filePath)}.zirk-live.zrk`);
}

function removeLiveTempFile(filePath) {
    const tempPath = liveTempPath(filePath);
    liveTempFiles.delete(tempPath);
    try {
        fs.unlinkSync(tempPath);
    } catch {
        // temp file may not exist
    }
}

function scheduleLiveCheck(document) {
    const key = document.uri.toString();
    const existing = pendingLiveChecks.get(key);
    if (existing) {
        clearTimeout(existing);
    }

    const config = vscode.workspace.getConfiguration(CONFIG_SECTION, document.uri);
    const delay = Math.max(100, config.get('checkDelay', 500));

    const timer = setTimeout(() => {
        pendingLiveChecks.delete(key);
        liveCheckDocument(document);
    }, delay);
    pendingLiveChecks.set(key, timer);
}

function liveCheckDocument(document) {
    if (languageClient) {
        // The server owns diagnostics for this document.
        return;
    }
    const key = document.uri.toString();
    if (runningLiveChecks.has(key)) {
        scheduleLiveCheck(document);
        return;
    }

    const executable = findCompiler(document.uri, 'check');
    if (!executable) {
        return;
    }

    const filePath = document.uri.fsPath;
    const tempPath = liveTempPath(filePath);

    try {
        fs.writeFileSync(tempPath, document.getText());
    } catch (error) {
        outputChannel.appendLine(`Could not write the live-check temp file: ${error.message}`);
        return;
    }
    liveTempFiles.add(tempPath);
    runningLiveChecks.add(key);

    runCheck(document, executable, tempPath, () => {
        runningLiveChecks.delete(key);
        removeLiveTempFile(filePath);
    });
}

function checkDocument(document, manual = false) {
    if (languageClient || document.languageId !== LANGUAGE_ID) {
        return;
    }

    if (document.isUntitled || document.uri.scheme !== 'file') {
        return;
    }

    if (!manual) {
        const config = vscode.workspace.getConfiguration(CONFIG_SECTION, document.uri);
        if (!config.get('checkOnSave', true)) {
            return;
        }
    }

    const executable = findCompiler(document.uri, 'check');
    if (!executable) {
        outputChannel.appendLine('No zirk or zirk-check executable found.');
        outputChannel.appendLine('Set zirk.executablePath or build the compiler with `cargo build -p zirk-cli --bin zirk-check --no-default-features`.');
        updateStatusBar(document.uri, false, 'no compiler');
        return;
    }

    const filePath = document.uri.fsPath;
    runCheck(document, executable, filePath);
}

function runBuildOrRun(document, action) {
    if (document.languageId !== LANGUAGE_ID || document.isUntitled || document.uri.scheme !== 'file') {
        return;
    }

    const executable = findCompiler(document.uri, 'build');
    if (!executable) {
        outputChannel.appendLine(`Cannot ${action}: the full zirk executable was not found.`);
        outputChannel.appendLine('Build it with the backend feature enabled, e.g. `cargo build -p zirk-cli --bin zirk`.');
        return;
    }

    const filePath = document.uri.fsPath;
    if (action === 'run') {
        runInTerminal(executable, document);
        return;
    }

    const commandName = 'Zirk: Build File';
    outputChannel.appendLine(`> ${executable} ${action} ${filePath}`);

    const child = spawn(executable, [action, filePath], {
        cwd: path.dirname(filePath),
    });

    let stdout = '';
    let stderr = '';

    child.stdout.on('data', chunk => {
        const text = chunk.toString();
        stdout += text;
        outputChannel.append(text);
    });

    child.stderr.on('data', chunk => {
        const text = chunk.toString();
        stderr += text;
        outputChannel.append(text);
    });

    child.on('error', error => {
        vscode.window.showErrorMessage(`${commandName} failed: ${error.message}`);
    });

    child.on('close', code => {
        outputChannel.appendLine(`(exit ${code ?? 'unknown'})`);

        if (action === 'build' && code === 0) {
            const builtPath = stdout.trim().split('\n').pop();
            if (builtPath) {
                vscode.window.showInformationMessage(`Built ${builtPath}`);
            }
        }
    });
}

/** Reused across runs; recreated when the user closes the panel. */
let runTerminal;

function shellQuote(value) {
    return `'${value.replace(/'/g, `'\\''`)}'`;
}

/** Runs `zirk run <file>` in an integrated terminal, like the Python play button. */
async function runInTerminal(executable, document) {
    if (document.isDirty) {
        await document.save();
    }
    if (!runTerminal || runTerminal.exitStatus !== undefined) {
        runTerminal = vscode.window.createTerminal({ name: 'Zirk', cwd: path.dirname(document.uri.fsPath) });
    }
    runTerminal.show();
    runTerminal.sendText(`${shellQuote(executable)} run ${shellQuote(document.uri.fsPath)}`);
}

/**
 * Finds the `zirk-lsp` executable: `zirk.lspPath` first, then the same search
 * roots the compiler discovery uses (`target/{debug,release}/zirk-lsp`), then
 * PATH.
 */
function findLanguageServer() {
    const root = vscode.workspace.workspaceFolders?.[0]?.uri ?? vscode.Uri.file(process.cwd());
    const config = vscode.workspace.getConfiguration(CONFIG_SECTION, root);
    const configured = config.get('lspPath', '');
    if (configured) {
        if (fs.existsSync(configured)) {
            return configured;
        }
        outputChannel.appendLine(`Configured zirk.lspPath not found: ${configured}`);
    }
    return findInSearchRoots(collectSearchRoots(root), ['zirk-lsp'])
        ?? findOnAnyPath(['zirk-lsp']);
}

/**
 * Spawns `zirk-lsp` through vscode-languageclient. Returns true when the
 * client is running — the CLI bridge then stays silent for `zirk` documents.
 */
async function startLanguageClient(context) {
    let LanguageClient;
    try {
        ({ LanguageClient } = require('vscode-languageclient/node'));
    } catch (error) {
        outputChannel.appendLine(`vscode-languageclient is not bundled: ${error.message}`);
        outputChannel.appendLine('Falling back to the CLI bridge.');
        return false;
    }

    const executable = findLanguageServer();
    if (!executable) {
        outputChannel.appendLine('No zirk-lsp executable found; falling back to the CLI bridge.');
        outputChannel.appendLine('Build it with `cargo build -p zirk-lsp`.');
        return false;
    }

    const client = new LanguageClient(
        'zirk-lsp',
        'Zirk Language Server',
        { command: executable },
        { documentSelector: [{ scheme: 'file', language: LANGUAGE_ID }] }
    );

    try {
        await client.start();
    } catch (error) {
        outputChannel.appendLine(`zirk-lsp failed to start: ${error.message ?? error}`);
        outputChannel.appendLine('Falling back to the CLI bridge.');
        return false;
    }

    context.subscriptions.push({ dispose: () => client.stop() });
    languageClient = client;
    outputChannel.appendLine(`zirk-lsp running: ${executable}`);
    return true;
}

function collectSearchRoots(documentUri) {
    const roots = [];
    const workspaceFolder = vscode.workspace.getWorkspaceFolder(documentUri);
    if (workspaceFolder) {
        roots.push(workspaceFolder.uri.fsPath);
    }

    let dir = path.dirname(documentUri.fsPath);
    for (let i = 0; i < 8; i += 1) {
        roots.push(dir);
        const parent = path.dirname(dir);
        if (parent === dir) {
            break;
        }
        dir = parent;
    }

    return [...new Set(roots)];
}

function findCompiler(documentUri, mode) {
    const config = vscode.workspace.getConfiguration(CONFIG_SECTION, documentUri);
    const configured = config.get('executablePath', '');
    if (configured) {
        if (fs.existsSync(configured)) {
            return configured;
        }
        outputChannel.appendLine(`Configured zirk.executablePath not found: ${configured}`);
    }

    const names = mode === 'check' ? ['zirk-check', 'zirk'] : ['zirk'];
    return findInSearchRoots(collectSearchRoots(documentUri), names)
        ?? findOnAnyPath(names);
}

function findInSearchRoots(roots, names) {
    for (const root of roots) {
        for (const name of names) {
            for (const profile of ['debug', 'release']) {
                const candidate = path.join(root, 'target', profile, name);
                if (fs.existsSync(candidate)) {
                    return candidate;
                }
            }
        }
    }
    return undefined;
}

function findOnAnyPath(names) {
    const pathDirs = (process.env.PATH || '').split(path.delimiter);
    for (const name of names) {
        const found = findOnPath(name, pathDirs);
        if (found) {
            return found;
        }
    }
    return undefined;
}

function findOnPath(name, directories) {
    const withExt = process.platform === 'win32' ? name + '.exe' : name;
    for (const directory of directories) {
        const candidate = path.join(directory, withExt);
        try {
            const stat = fs.statSync(candidate);
            if (stat.isFile()) {
                return candidate;
            }
        } catch {
            // candidate does not exist; continue
        }
    }
    return undefined;
}

function runCheck(document, executable, filePath, cleanup) {
    const baseName = path.basename(executable, path.extname(executable));
    const isZirkCheck = baseName === 'zirk-check';
    const args = isZirkCheck ? [filePath, '--json'] : ['check', '--json', filePath];

    outputChannel.appendLine(`> ${executable} ${args.join(' ')}`);

    const child = spawn(executable, args, {
        cwd: path.dirname(filePath),
    });

    let stderr = '';
    let stdout = '';

    child.stdout.on('data', chunk => {
        const text = chunk.toString();
        stdout += text;
        outputChannel.append(text);
    });

    child.stderr.on('data', chunk => {
        const text = chunk.toString();
        stderr += text;
    });

    child.on('error', error => {
        outputChannel.appendLine(`Failed to run the Zirk compiler: ${error.message}`);
        updateStatusBar(document.uri, false, 'check failed');
        if (cleanup) {
            cleanup();
        }
    });

    child.on('close', code => {
        if (stderr) {
            outputChannel.append(stderr);
        }
        outputChannel.appendLine(`(exit ${code ?? 'unknown'})`);

        const diagnostics = parseDiagnostics(document, stderr);
        diagnosticCollection.set(document.uri, diagnostics);

        if (code === 0 && diagnostics.length === 0) {
            updateStatusBar(document.uri, true, 'OK');
        } else if (diagnostics.length > 0) {
            const count = diagnostics.filter(d => d.severity === vscode.DiagnosticSeverity.Error).length;
            const label = count === 1 ? '1 error' : `${count} errors`;
            updateStatusBar(document.uri, false, label);
        } else {
            updateStatusBar(document.uri, false, 'check failed');
        }

        refreshTypeMap(document, executable, filePath);

        if (cleanup) {
            cleanup();
        }
    });
}

/**
 * Refreshes the cached type map for `document` by invoking the same
 * executable the diagnostic check used, with `--types` (`editor-cli-bridge`).
 * A failed or malformed response leaves the previous cache in place — the
 * bridge reports no hover rather than an incorrect one.
 */
function refreshTypeMap(document, executable, filePath) {
    const baseName = path.basename(executable, path.extname(executable));
    const isZirkCheck = baseName === 'zirk-check';
    const args = isZirkCheck ? [filePath, '--types'] : ['check', '--types', filePath];

    const child = spawn(executable, args, {
        cwd: path.dirname(filePath),
    });

    let stdout = '';
    child.stdout.on('data', chunk => {
        stdout += chunk.toString();
    });

    child.on('error', () => {
        // keep the previous map
    });
    child.on('close', code => {
        if (code !== 0 || !stdout.trim()) {
            return;
        }
        let parsed;
        try {
            parsed = JSON.parse(stdout);
        } catch {
            return;
        }
        if (!parsed || !Array.isArray(parsed.types)) {
            return;
        }
        const entries = parsed.types.filter(entry =>
            entry
            && typeof entry.start === 'number'
            && typeof entry.end === 'number'
            && typeof entry.kind === 'string'
            && typeof entry.type === 'string'
        );
        typeMaps.set(document.uri.toString(), entries);
    });
}

/**
 * Answers a hover from the cached `zirk check --types` map: the smallest
 * span containing the position wins, declaration kinds beat expressions on
 * a tie (`editor-cli-bridge`, "Hover mapping").
 */
function provideTypeHover(document, position) {
    const entries = typeMaps.get(document.uri.toString());
    if (!entries) {
        return undefined;
    }

    const offset = document.offsetAt(position);
    let best;
    for (const entry of entries) {
        if (entry.start > offset || offset > entry.end) {
            continue;
        }
        const better = !best
            || (entry.end - entry.start) < (best.end - best.start)
            || ((entry.end - entry.start) === (best.end - best.start) && best.kind === 'expression');
        if (better) {
            best = entry;
        }
    }
    if (!best) {
        return undefined;
    }

    let display;
    if (best.kind === 'function' && best.name) {
        display = `fn ${best.name}${best.type.slice(2)}`;
    } else if ((best.kind === 'method' || best.kind === 'variable'
        || best.kind === 'parameter' || best.kind === 'field') && best.name) {
        display = best.kind === 'method'
            ? `${best.name}${best.type.slice(2)}`
            : `${best.name}: ${best.type}`;
    } else {
        display = best.type;
    }

    const markdown = new vscode.MarkdownString();
    markdown.appendCodeblock(display, 'zirk');
    if (best.inferred) {
        markdown.appendMarkdown('\n\n*(inferred)*');
    }
    return new vscode.Hover(markdown);
}

function updateStatusBar(documentUri, ok, detail) {
    if (!statusBarItem) {
        return;
    }

    const activeEditor = vscode.window.activeTextEditor;
    if (!activeEditor || activeEditor.document.uri.toString() !== documentUri.toString()) {
        return;
    }

    const icon = ok ? '$(check)' : '$(warning)';
    statusBarItem.text = `${icon} Zirk: ${detail}`;
    statusBarItem.show();
}

function updateStatusBarForDocument(document) {
    if (!statusBarItem || languageClient) {
        return;
    }

    const existing = diagnosticCollection.get(document.uri);
    if (!existing || existing.length === 0) {
        statusBarItem.text = '$(check) Zirk: OK';
    } else {
        const count = existing.filter(d => d.severity === vscode.DiagnosticSeverity.Error).length;
        const label = count === 1 ? '1 error' : `${count} errors`;
        statusBarItem.text = `$(warning) Zirk: ${label}`;
    }
    statusBarItem.show();
}

function parseDiagnostics(document, stderr) {
    const text = stderr.trim();
    if (!text) {
        return [];
    }

    let items;
    try {
        items = JSON.parse(text);
    } catch (error) {
        outputChannel.appendLine(`Could not parse compiler output as JSON: ${error.message}`);
        return [];
    }

    if (!Array.isArray(items)) {
        outputChannel.appendLine('Compiler output was not a JSON array of diagnostics.');
        return [];
    }

    return items
        .map(item => diagnosticFromJson(document, item))
        .filter(diagnostic => diagnostic !== undefined);
}

function diagnosticFromJson(document, item) {
    if (!item || typeof item.message !== 'string') {
        return undefined;
    }

    const severity = item.severity === 'error'
        ? vscode.DiagnosticSeverity.Error
        : vscode.DiagnosticSeverity.Warning;

    const range = locationToRange(document, item.location);
    let message = item.message;
    if (item.cause) {
        message += `\n\ncause: ${item.cause}`;
    }
    if (item.help) {
        message += `\n\nhelp: ${item.help}`;
    }

    const diagnostic = new vscode.Diagnostic(range, message, severity);
    diagnostic.source = DIAGNOSTIC_SOURCE;
    if (item.code) {
        diagnostic.code = item.code;
    }
    return diagnostic;
}

function locationToRange(document, location) {
    if (!location || typeof location.line !== 'number' || typeof location.column !== 'number') {
        return new vscode.Range(0, 0, 0, 0);
    }

    const lineIndex = Math.max(0, location.line - 1);
    if (lineIndex >= document.lineCount) {
        const lastLine = Math.max(0, document.lineCount - 1);
        const lastText = document.lineAt(lastLine).text;
        return new vscode.Range(lastLine, 0, lastLine, lastText.length);
    }

    const lineText = document.lineAt(lineIndex).text;
    const charCount = Array.from(lineText).length;
    let charIndex = Math.max(0, location.column - 1);

    if (charIndex >= charCount) {
        charIndex = Math.max(0, charCount - 1);
    }

    const startOffset = charToUtf16Offset(lineText, charIndex);
    const endOffset = charToUtf16Offset(lineText, charIndex + 1);
    return new vscode.Range(lineIndex, startOffset, lineIndex, endOffset);
}

function charToUtf16Offset(text, charIndex) {
    let offset = 0;
    let remaining = charIndex;
    for (const ch of text) {
        if (remaining <= 0) {
            break;
        }
        remaining--;
        offset += ch.length;
    }
    return offset;
}

function provideZirkTasks() {
    const workspaceFolder = vscode.workspace.workspaceFolders?.[0];
    if (!workspaceFolder) {
        return [];
    }

    const tasks = [];
    for (const action of ['build', 'run']) {
        const definition = {
            type: 'zirk',
            task: action,
        };
        const task = new vscode.Task(
            definition,
            workspaceFolder,
            `${action} current file`,
            'zirk',
            undefined
        );
        task.group = action === 'build' ? vscode.TaskGroup.Build : undefined;
        tasks.push(task);
    }
    return tasks;
}

function resolveZirkTask(task) {
    if (task.definition.type !== 'zirk') {
        return undefined;
    }

    const workspaceFolder = vscode.workspace.workspaceFolders?.[0];
    if (!workspaceFolder) {
        return undefined;
    }

    const action = task.definition.task;
    if (action !== 'build' && action !== 'run') {
        return undefined;
    }

    const executable = findCompilerForTasks(workspaceFolder.uri);
    if (!executable) {
        return undefined;
    }

    const command = `"${executable}" ${action} \${fileBasenameNoExtension}`;
    const execution = new vscode.ShellExecution(command, {
        cwd: workspaceFolder.uri.fsPath,
    });

    const resolved = new vscode.Task(
        task.definition,
        workspaceFolder,
        task.name || `${action} current file`,
        'zirk',
        execution
    );
    resolved.group = action === 'build' ? vscode.TaskGroup.Build : undefined;
    return resolved;
}

function findCompilerForTasks(workspaceUri) {
    const config = vscode.workspace.getConfiguration(CONFIG_SECTION, workspaceUri);
    const configured = config.get('executablePath', '');
    if (configured && fs.existsSync(configured)) {
        return configured;
    }

    const root = workspaceUri.fsPath;
    const candidates = [
        path.join(root, 'target', 'debug', 'zirk'),
        path.join(root, 'target', 'release', 'zirk'),
    ];
    for (const candidate of candidates) {
        if (fs.existsSync(candidate)) {
            return candidate;
        }
    }

    const pathDirs = (process.env.PATH || '').split(path.delimiter);
    return findOnPath('zirk', pathDirs);
}

module.exports = {
    activate,
    deactivate
};
