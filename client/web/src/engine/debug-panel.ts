import GUI from 'lil-gui';

/**
 * Debug panel using lil-gui for displaying runtime values.
 * Creates a collapsible panel in the top-right corner.
 */
export class DebugPanel {
    private readonly gui: GUI;
    private readonly scopes = new Map<string, GUI>();
    private readonly scopeValues = new Map<string, Map<string, { value: string }>>();
    private gameContainer: HTMLElement | null = null;

    constructor() {
        this.gui = new GUI({ title: 'Debug' });
        this.gui.domElement.style.position = 'absolute';
        this.gui.domElement.style.top = '10px';
        this.gui.domElement.style.right = '10px';

        this.gui.domElement.addEventListener('mousedown', this.handleMouseDown);
        this.gui.domElement.addEventListener('click', this.handleClick);
    }

    setGameContainer(container: HTMLElement): void {
        this.gameContainer = container;
    }

    private handleMouseDown = (_ev: MouseEvent): void => {
        // Allow the click to proceed normally, but refocus after a brief delay
        setTimeout(() => {
            this.refocusGame();
        }, 0);
    };

    private handleClick = (): void => {
        this.refocusGame();
    };

    private refocusGame(): void {
        if (this.gameContainer) {
            this.gameContainer.focus();
        }
    }

    /**
     * Get or create a scope (folder) for a component.
     * Each scope is a collapsible section in the debug panel.
     */
    private getScope(scope: string): GUI {
        let folder = this.scopes.get(scope);
        if (!folder) {
            folder = this.gui.addFolder(scope);
            this.scopes.set(scope, folder);
            this.scopeValues.set(scope, new Map());
        }
        return folder;
    }

    /**
     * Set a debug value within a scope.
     * Creates a new controller if the key doesn't exist.
     */
    set(scope: string, key: string, value: string): void {
        const folder = this.getScope(scope);
        const values = this.scopeValues.get(scope)!;

        let obj = values.get(key);
        if (!obj) {
            obj = { value };
            values.set(key, obj);
            folder.add(obj, 'value').name(key).disable().listen();
        } else {
            obj.value = value;
        }
    }

    /**
     * Add a boolean toggle control within a scope.
     */
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    addToggle(scope: string, name: string, object: any, property: string): void {
        const folder = this.getScope(scope);
        folder.add(object, property).name(name);
    }

    /**
     * Add a button control within a scope.
     */
    addButton(scope: string, name: string, callback: () => void): void {
        const folder = this.getScope(scope);
        folder.add({ action: callback }, 'action').name(name);
    }

    /**
     * Remove all debug entries for a scope and remove the folder.
     */
    removeScope(scope: string): void {
        const folder = this.scopes.get(scope);
        if (folder) {
            folder.destroy();
            this.scopes.delete(scope);
            this.scopeValues.delete(scope);
        }
    }

    /**
     * Show the debug panel.
     */
    show(): void {
        this.gui.show();
    }

    /**
     * Hide the debug panel.
     */
    hide(): void {
        this.gui.hide();
    }

    dispose(): void {
        this.gui.domElement.removeEventListener('mousedown', this.handleMouseDown);
        this.gui.domElement.removeEventListener('click', this.handleClick);
        this.gui.destroy();
        this.scopes.clear();
        this.scopeValues.clear();
    }
}
