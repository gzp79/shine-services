import GUI from 'lil-gui';

export class DebugPanel {
    private readonly gui: GUI;
    private readonly scopes = new Map<string, GUI>();
    private readonly scopeValues = new Map<string, Map<string, { value: string }>>();
    private readonly gameContainer: HTMLElement;

    constructor(container: HTMLElement, title = 'Debug') {
        this.gameContainer = container;
        this.gui = new GUI({ title });
        this.gui.domElement.style.position = 'absolute';
        this.gui.domElement.style.top = '10px';
        this.gui.domElement.style.right = '10px';

        this.gui.domElement.addEventListener('mousedown', this.handleMouseDown);
        this.gui.domElement.addEventListener('click', this.handleClick);
    }

    show(): void {
        this.gui.show();
    }

    hide(): void {
        this.gui.hide();
    }

    root(): GUI {
        return this.gui;
    }

    scope(name: string): GUI {
        let folder = this.scopes.get(name);
        if (!folder) {
            folder = this.gui.addFolder(name);
            this.scopes.set(name, folder);
            this.scopeValues.set(name, new Map());
        }
        return folder;
    }

    removeScope(name: string): void {
        const folder = this.scopes.get(name);
        if (folder) {
            folder.destroy();
            this.scopes.delete(name);
            this.scopeValues.delete(name);
        }
    }

    set(scope: string, key: string, value: string): void {
        const folder = this.scope(scope);
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

    setValues(scope: string, record: Record<string, string>): void {
        const values = this.scopeValues.get(scope);
        if (values) {
            for (const key of values.keys()) {
                if (!(key in record)) this._removeKey(scope, key);
            }
        }
        for (const [key, value] of Object.entries(record)) {
            this.set(scope, key, value);
        }
    }

    private _removeKey(scope: string, key: string): void {
        const folder = this.scopes.get(scope);
        const values = this.scopeValues.get(scope);
        if (!folder || !values) return;
        const ctrl = folder.controllers.find((c) => c.property === 'value' && c._name === key);
        ctrl?.destroy();
        values.delete(key);
    }

    dispose(): void {
        this.gui.domElement.removeEventListener('mousedown', this.handleMouseDown);
        this.gui.domElement.removeEventListener('click', this.handleClick);
        this.gui.destroy();
        this.scopes.clear();
        this.scopeValues.clear();
    }

    private handleMouseDown = (_ev: MouseEvent): void => {
        setTimeout(() => this.gameContainer.focus(), 0);
    };

    private handleClick = (): void => {
        this.gameContainer.focus();
    };
}
