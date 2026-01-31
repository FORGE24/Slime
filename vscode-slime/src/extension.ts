import * as vscode from 'vscode';
import * as child_process from 'child_process';
import * as path from 'path';

export function activate(context: vscode.ExtensionContext) {
    let disposable = vscode.commands.registerCommand('slime.build', () => {
        const editor = vscode.window.activeTextEditor;
        if (!editor) {
            vscode.window.showErrorMessage('No active editor');
            return;
        }

        const filePath = editor.document.uri.fsPath;
        if (!filePath.endsWith('.sm')) {
            vscode.window.showErrorMessage('Active file is not a Slime file');
            return;
        }

        const terminal = vscode.window.createTerminal('Slime Build');
        terminal.sendText(`slime build ${filePath}`);
        terminal.show();
    });

    context.subscriptions.push(disposable);
}

export function deactivate() {}
