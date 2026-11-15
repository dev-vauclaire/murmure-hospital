import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { parseMicrophoneControls } from '../microphone-controls.helpers';
import type { MicrophoneControlsSnapshot, MicrophoneControlsState } from '../microphone-controls.types';

export const useMicrophoneControlsState = (): MicrophoneControlsState => {
    const [controls, setControls] = useState<MicrophoneControlsSnapshot | null>(null);
    const [loadFailed, setLoadFailed] = useState(false);
    const [attempt, setAttempt] = useState(0);

    useEffect(() => {
        let active = true;
        let updated = false;
        let unlisten: UnlistenFn | undefined;

        const load = async () => {
            try {
                unlisten = await listen<unknown>('microphone-controls-changed', ({ payload }) => {
                    if (!active) return;
                    const next = parseMicrophoneControls(payload);
                    if (next === null) return;
                    updated = true;
                    setControls(next);
                    setLoadFailed(false);
                });
                if (!active) {
                    unlisten();
                    unlisten = undefined;
                    return;
                }
                const snapshot = await invoke<unknown>('get_microphone_controls');
                // The live event may describe a newer profile than this snapshot.
                if (!active || updated) return;
                const next = parseMicrophoneControls(snapshot);
                if (next === null) {
                    setLoadFailed(true);
                    return;
                }
                setControls(next);
                setLoadFailed(false);
            } catch {
                if (active && !updated) setLoadFailed(true);
            }
        };

        void load();
        return () => {
            active = false;
            unlisten?.();
        };
    }, [attempt]);

    return {
        controls,
        loadFailed,
        retry: () => {
            setLoadFailed(false);
            setAttempt((value) => value + 1);
        },
    };
};
