import * as vscode from 'vscode';

/**
 * Chat session for persistence.
 */
export interface Session {
    id: string;
    name: string;
    messages: any[];
    createdAt: number;
    updatedAt: number;
    projectRoot?: string;
}

/**
 * Manages chat sessions with persistence using VSCode globalState.
 */
export class SessionManager {
    private context: vscode.ExtensionContext;
    private storageKey = 'matrixcode.sessions';
    
    constructor(context: vscode.ExtensionContext) {
        this.context = context;
    }
    
    /**
     * Get all saved sessions.
     */
    async getSessions(): Promise<Session[]> {
        const sessions = this.context.globalState.get<Session[]>(this.storageKey, []);
        return sessions.sort((a, b) => b.updatedAt - a.updatedAt); // Most recent first
    }
    
    /**
     * Save a session.
     */
    async saveSession(session: Session): Promise<void> {
        const sessions = await this.getSessions();
        
        // Update existing or add new
        const existingIndex = sessions.findIndex(s => s.id === session.id);
        if (existingIndex >= 0) {
            sessions[existingIndex] = session;
        } else {
            sessions.push(session);
        }
        
        // Keep only last 50 sessions
        const trimmed = sessions.slice(0, 50);
        
        await this.context.globalState.update(this.storageKey, trimmed);
    }
    
    /**
     * Delete a session by ID.
     */
    async deleteSession(id: string): Promise<void> {
        const sessions = await this.getSessions();
        const filtered = sessions.filter(s => s.id !== id);
        await this.context.globalState.update(this.storageKey, filtered);
    }
    
    /**
     * Create a new session with auto-generated name.
     */
    createSession(messages: any[] = [], projectRoot?: string): Session {
        const now = Date.now();
        const date = new Date(now);
        const name = `Session ${date.toLocaleDateString()} ${date.toLocaleTimeString()}`;
        
        return {
            id: this.generateId(),
            name,
            messages,
            createdAt: now,
            updatedAt: now,
            projectRoot
        };
    }
    
    /**
     * Generate unique ID.
     */
    private generateId(): string {
        return Date.now().toString(36) + Math.random().toString(36).substr(2);
    }
    
    /**
     * Show session picker for loading.
     */
    async showSessionPicker(): Promise<Session | undefined> {
        const sessions = await this.getSessions();
        
        if (sessions.length === 0) {
            vscode.window.showInformationMessage('No saved sessions found');
            return undefined;
        }
        
        const items = sessions.map(s => ({
            label: s.name,
            description: `${s.messages.length} messages - ${new Date(s.updatedAt).toLocaleDateString()}`,
            session: s
        }));
        
        const selected = await vscode.window.showQuickPick(items, {
            placeHolder: 'Select a session to load'
        });
        
        return selected?.session;
    }
    
    /**
     * Prompt user to name a session.
     */
    async promptSessionName(defaultName?: string): Promise<string | undefined> {
        return await vscode.window.showInputBox({
            prompt: 'Enter session name',
            value: defaultName || '',
            placeHolder: 'My Session'
        });
    }
}
