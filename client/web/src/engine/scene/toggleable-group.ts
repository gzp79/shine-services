import * as THREE from 'three';
import { disposeObject3D } from '../resources/ownership';

export interface ToggleableGroup {
    group: THREE.Group;
    setVisible: (visible: boolean) => void;
    setIndividualVisible: (index: number, visible: boolean) => void;
    dispose: () => void;
}

export function createToggleableGroup(group: THREE.Group, meshGroups: THREE.Group[]): ToggleableGroup {
    return {
        group,
        setVisible: (visible: boolean) => meshGroups.forEach((meshGroup) => (meshGroup.visible = visible)),
        setIndividualVisible: (index: number, visible: boolean) => {
            if (index >= 0 && index < meshGroups.length) meshGroups[index].visible = visible;
        },
        dispose: () => meshGroups.forEach((meshGroup) => disposeObject3D(meshGroup))
    };
}
