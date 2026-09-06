const vscode = require('vscode');

const invalidToReplacement = {
    'BinaryFloat': 'Float',
    'BinaryFloat16': 'Float16',
    'BinaryFloat32': 'Float32',
    'BinaryFloat64': 'Float64',
    'BinaryFloat128': 'Float128',
    'Decimal16': 'Decimal',
    'Decimal32': 'Decimal',
    'Decimal64': 'Decimal',
    'Decimal128': 'Decimal'
};

const invalidTypePattern = new RegExp(
    '\\b(' + Object.keys(invalidToReplacement).join('|') + ')\\b',
    'g'
);

function activate(context) {
    const diagnosticCollection = vscode.languages.createDiagnosticCollection('zirk');

    function updateDiagnostics(document) {
        if (document.languageId !== 'zirk') {
            return;
        }

        const text = document.getText();
        const diagnostics = [];

        let match;
        invalidTypePattern.lastIndex = 0;
        while ((match = invalidTypePattern.exec(text)) !== null) {
            const oldName = match[1];
            const replacement = invalidToReplacement[oldName];
            const start = document.positionAt(match.index);
            const end = document.positionAt(match.index + oldName.length);
            const range = new vscode.Range(start, end);
            const message = `Type name "${oldName}" is no longer valid. Use "${replacement}" instead.`;
            const diagnostic = new vscode.Diagnostic(
                range,
                message,
                vscode.DiagnosticSeverity.Error
            );
            diagnostic.code = 'zirk-deprecated-type';
            diagnostics.push(diagnostic);
        }

        diagnosticCollection.set(document.uri, diagnostics);
    }

    context.subscriptions.push(diagnosticCollection);

    const openDisposable = vscode.workspace.onDidOpenTextDocument(updateDiagnostics);
    const changeDisposable = vscode.workspace.onDidChangeTextDocument(event => updateDiagnostics(event.document));
    const closeDisposable = vscode.workspace.onDidCloseTextDocument(document => diagnosticCollection.delete(document.uri));

    context.subscriptions.push(openDisposable, changeDisposable, closeDisposable);

    for (const document of vscode.workspace.textDocuments) {
        updateDiagnostics(document);
    }

    const formatter = vscode.languages.registerDocumentFormattingEditProvider('zirk', {
        provideDocumentFormattingEdits(document, options, token) {
            const textEdits = [];
            const lineCount = document.lineCount;
            let indentLevel = 0;
            const tabSize = options.tabSize || 4;
            const insertSpaces = options.insertSpaces !== false;
            const indentStr = insertSpaces ? ' '.repeat(tabSize) : '\t';

            for (let i = 0; i < lineCount; i++) {
                const line = document.lineAt(i);
                let text = line.text.trim();

                // If the line starts with a closing brace, decrease indent level first
                if (text.startsWith('}') || text.startsWith(']')) {
                    indentLevel = Math.max(0, indentLevel - 1);
                }

                // Construct new formatted line
                let formatted = text;
                if (text.length > 0) {
                    formatted = indentStr.repeat(indentLevel) + text;
                }

                // If the line ends with an opening brace or bracket, increase indent level for next lines
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
    });

    context.subscriptions.push(formatter);
}

function deactivate() {}

module.exports = {
    activate,
    deactivate
};
