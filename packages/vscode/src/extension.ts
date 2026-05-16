/**
 * MatrixCode VSCode Extension Entry Point
 */

import * as vscode from 'vscode';
import { MatrixCodeClient } from './matrixcodeClient';
import { ChatViewProvider } from './chatView';
import { ConfigManager } from './configManager';

let client: MatrixCodeClient;
let chatProvider: ChatViewProvider;
let configManager: ConfigManager;

export async function activate(context: vscode.ExtensionContext): Promise<void> {
    console.log('MatrixCode extension is activating...');
    
    // Initialize configuration manager
    configManager = new ConfigManager();
    
    // Initialize CLI client
    client = new MatrixCodeClient(configManager);
    
    // Register chat view
    chatProvider = new ChatViewProvider(context.extensionUri, client, configManager);
    context.subscriptions.push(
        vscode.window.registerWebviewViewProvider('matrixcode.chat', chatProvider)
    );
    
    // Register all commands
    registerCommands(context);
    
    // Check CLI availability
    const available = await client.checkAvailability();
    if (!available) {
        const result = await vscode.window.showWarningMessage(
            'MatrixCode CLI not found. The extension requires the MatrixCode CLI to function.',
            'Install CLI',
            'Configure Path',
            'Ignore'
        );
        
        handleCliNotFound(result);
    } else {
        console.log('MatrixCode CLI found, starting daemon...');
        
        // Start daemon if enabled
        if (configManager.get('daemonMode')) {
            try {
                await client.startDaemon();
                vscode.window.setStatusBarMessage('$(check) MatrixCode connected', 3000);
            } catch (error) {
                const errMsg = error instanceof Error ? error.message : String(error);
                vscode.window.showErrorMessage(`Failed to start MatrixCode daemon: ${errMsg}`);
            }
        }
    }
    
    // Listen for configuration changes
    context.subscriptions.push(
        vscode.workspace.onDidChangeConfiguration(e => {
            if (e.affectsConfiguration('matrixcode')) {
                configManager.reload();
                client.updateConfig(configManager);
            }
        })
    );
    
    console.log('MatrixCode extension activated');
}

function registerCommands(context: vscode.ExtensionContext): void {
    // Explain code command
    context.subscriptions.push(
        vscode.commands.registerCommand('matrixcode.explain', async () => {
            const editor = vscode.window.activeTextEditor;
            if (!editor) {
                vscode.window.showWarningMessage('No active editor');
                return;
            }
            
            const selection = editor.selection;
            if (selection.isEmpty) {
                vscode.window.showWarningMessage('Please select some code to explain');
                return;
            }
            
            const text = editor.document.getText(selection);
            const context = buildEditorContext(editor);
            
            await chatProvider.sendQuickAction('explain', text, context);
        })
    );
    
    // Fix code command
    context.subscriptions.push(
        vscode.commands.registerCommand('matrixcode.fix', async () => {
            const editor = vscode.window.activeTextEditor;
            if (!editor) {
                vscode.window.showWarningMessage('No active editor');
                return;
            }
            
            const selection = editor.selection;
            if (selection.isEmpty) {
                vscode.window.showWarningMessage('Please select some code to fix');
                return;
            }
            
            const text = editor.document.getText(selection);
            const context = buildEditorContext(editor);
            
            // Add diagnostics to context
            const diagnostics = vscode.languages.getDiagnostics(editor.document.uri);
            if (diagnostics.length > 0) {
                const relevantDiagnostics = diagnostics.filter(d => {
                    const range = d.range;
                    return selection.contains(range.start) || selection.contains(range.end);
                });
                if (relevantDiagnostics.length > 0) {
                    context.diagnostics = relevantDiagnostics.map(d => ({
                        severity: vscode.DiagnosticSeverity[d.severity].toLowerCase(),
                        message: d.message,
                        range: {
                            start: { line: d.range.start.line, character: d.range.start.character },
                            end: { line: d.range.end.line, character: d.range.end.character }
                        }
                    }));
                }
            }
            
            await chatProvider.sendQuickAction('fix', text, context);
        })
    );
    
    // Generate tests command
    context.subscriptions.push(
        vscode.commands.registerCommand('matrixcode.generateTests', async () => {
            const editor = vscode.window.activeTextEditor;
            if (!editor) {
                vscode.window.showWarningMessage('No active editor');
                return;
            }
            
            const selection = editor.selection;
            const text = selection.isEmpty 
                ? editor.document.getText() 
                : editor.document.getText(selection);
            const context = buildEditorContext(editor);
            
            await chatProvider.sendQuickAction('generateTests', text, context);
        })
    );
    
    // Refactor command
    context.subscriptions.push(
        vscode.commands.registerCommand('matrixcode.refactor', async () => {
            const editor = vscode.window.activeTextEditor;
            if (!editor) {
                vscode.window.showWarningMessage('No active editor');
                return;
            }
            
            const selection = editor.selection;
            if (selection.isEmpty) {
                vscode.window.showWarningMessage('Please select some code to refactor');
                return;
            }
            
            const text = editor.document.getText(selection);
            const context = buildEditorContext(editor);
            
            // Prompt for refactor instructions
            const instructions = await vscode.window.showInputBox({
                prompt: 'Refactor instructions (optional)',
                placeHolder: 'e.g., Extract method, Rename variables, etc.'
            });
            
            await chatProvider.sendQuickAction('refactor', text, context, instructions);
        })
    );
    
    // New session command
    context.subscriptions.push(
        vscode.commands.registerCommand('matrixcode.newSession', async () => {
            await client.newSession();
            chatProvider.clearHistory();
            vscode.window.showInformationMessage('MatrixCode: Started new session');
        })
    );
    
    // Show history command
    context.subscriptions.push(
        vscode.commands.registerCommand('matrixcode.showHistory', async () => {
            await vscode.commands.executeCommand('workbench.view.extension.matrixcode');
            // TODO: Show session history in webview
        })
    );
    
    // Open settings command
    context.subscriptions.push(
        vscode.commands.registerCommand('matrixcode.openSettings', () => {
            vscode.commands.executeCommand('workbench.action.openSettings', 'matrixcode');
        })
    );
}

function buildEditorContext(editor: vscode.TextEditor): EditorContext {
    const document = editor.document;
    const selection = editor.selection;
    
    return {
        workspace: vscode.workspace.workspaceFolders?.[0]?.uri.fsPath,
        file: document.uri.fsPath,
        language: document.languageId,
        selection: {
            start: { line: selection.start.line, character: selection.start.character },
            end: { line: selection.end.line, character: selection.end.character }
        }
    };
}

async function handleCliNotFound(result: string | undefined): Promise<void> {
    switch (result) {
        case 'Install CLI':
            await vscode.env.openExternal(
                vscode.Uri.parse('https://github.com/bigfish1913/matrixcode#installation')
            );
            break;
        case 'Configure Path':
            await vscode.commands.executeCommand('workbench.action.openSettings', 'matrixcode.cliPath');
            break;
        case 'Ignore':
            break;
    }
}

export function deactivate(): void {
    if (client) {
        client.dispose();
    }
    console.log('MatrixCode extension deactivated');
}

interface EditorContext {
    workspace?: string;
    file: string;
    language: string;
    selection: {
        start: { line: number; character: number };
        end: { line: number; character: number };
    };
    diagnostics?: Array<{
        severity: string;
        message: string;
        range: { start: { line: number; character: number }; end: { line: number; character: number } };
    }>;
}