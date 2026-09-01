import type GUI from 'lil-gui';
import type { AssetStore } from '../engine/assets/asset-store';

const NONE = 'none';
const FILE = 'file';

// Reacts to a source selection. The picker owns the dropdown, the file dialog and the picked
// file's object URL; the consumer only decides what to render for each source.
export interface AssetSourceHandlers {
    onNone(): void;
    onAsset(name: string): void;
    onFile(url: string, fileName: string): void;
}

// A debug-panel dropdown that selects a tile/model source: 'none', a picked file, or a catalog
// asset. 'file' is an action (never a resting value) that opens the dialog; a loaded file becomes
// its own option so re-picking 'file' loads a different one, and its object URL is kept alive so
// the file entry can be re-selected.
export class AssetSourcePicker {
    private readonly proxy = { asset: NONE };
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    private readonly ctrl: any;
    private names: string[] = [];
    private current = NONE;
    private fileUrl: string | null = null;
    private fileName: string | null = null;
    private readonly fileInput: HTMLInputElement;

    constructor(
        private readonly gui: GUI,
        private readonly store: AssetStore,
        private readonly handlers: AssetSourceHandlers
    ) {
        this.fileInput = document.createElement('input');
        this.fileInput.type = 'file';
        this.fileInput.accept = '.gltf,.glb';
        this.fileInput.style.display = 'none';
        document.body.appendChild(this.fileInput);
        this.fileInput.addEventListener('change', (e) => this.onFileChange(e));

        this.ctrl = this.gui
            .add(this.proxy, 'asset', [NONE])
            .name('Asset')
            .onChange((name: string) => this.onChange(name));
    }

    async populate(): Promise<void> {
        this.names = (await this.store.list()).map((a) => a.name);
        this.refresh();
    }

    dispose(): void {
        this.clearFile();
        this.ctrl.destroy();
        this.fileInput.remove();
    }

    // Repopulates the dropdown options in place (OptionController.options keeps the controller and
    // its position); does not fire onChange.
    private refresh(): void {
        const fileEntry = this.fileName ? [this.fileLabel(this.fileName)] : [];
        this.proxy.asset = this.current;
        this.ctrl.options([NONE, FILE, ...fileEntry, ...this.names]);
    }

    // Displayed as "file: <name>" in the dropdown to distinguish it from catalog assets.
    private fileLabel(name: string): string {
        return `${FILE}: ${name}`;
    }

    private onChange(name: string): void {
        if (name === FILE) {
            this.fileInput.click();
            this.proxy.asset = this.current;
            this.ctrl.updateDisplay();
            return;
        }
        this.current = name;
        if (this.fileName && name === this.fileLabel(this.fileName)) {
            this.handlers.onFile(this.fileUrl!, this.fileName);
            return;
        }
        // Leaving the file: drop its option so it no longer lingers in the list.
        const hadFile = this.fileName !== null;
        if (hadFile) this.clearFile();
        if (name === NONE) this.handlers.onNone();
        else this.handlers.onAsset(name);
        if (hadFile) this.refresh();
    }

    private onFileChange(e: Event): void {
        const input = e.target as HTMLInputElement;
        const file = input.files?.[0];
        input.value = '';
        if (!file) return;
        this.clearFile();
        this.fileUrl = URL.createObjectURL(file);
        this.fileName = file.name;
        this.current = this.fileLabel(file.name);
        this.refresh();
        this.handlers.onFile(this.fileUrl, this.fileName);
    }

    private clearFile(): void {
        if (this.fileUrl) URL.revokeObjectURL(this.fileUrl);
        this.fileUrl = null;
        this.fileName = null;
    }
}
