const vscode = require('vscode');

function activate(context) {
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
