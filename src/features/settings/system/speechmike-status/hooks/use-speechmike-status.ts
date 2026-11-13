import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { useEffect, useState } from 'react';

const SPEECHMIKE_STATUSES = [
    'unsupported',
    'searching',
    'initializing',
    'ready',
    'disconnected',
    'multiple_devices',
    'wrong_mode',
    'unsupported_device',
    'unavailable',
] as const;

export type SpeechMikeStatusValue = (typeof SPEECHMIKE_STATUSES)[number];

const isSpeechMikeStatus = (value: unknown): value is SpeechMikeStatusValue =>
    typeof value === 'string' && (SPEECHMIKE_STATUSES as readonly string[]).includes(value);

export const useSpeechMikeStatus = () => {
    const [status, setStatus] = useState<SpeechMikeStatusValue | null>(null);

    useEffect(() => {
        let active = true;
        let updated = false;
        let unlisten: UnlistenFn | undefined;

        const load = async () => {
            try {
                unlisten = await listen<unknown>('speechmike-status', ({ payload }) => {
                    if (!active) return;
                    updated = true;
                    setStatus(isSpeechMikeStatus(payload) ? payload : 'unavailable');
                });
                if (!active) {
                    unlisten();
                    unlisten = undefined;
                    return;
                }

                const initialStatus = await invoke<unknown>('get_speechmike_status');
                // A connection event may be newer than this initial snapshot.
                if (active && !updated) {
                    setStatus(isSpeechMikeStatus(initialStatus) ? initialStatus : 'unavailable');
                }
            } catch {
                if (active && !updated) setStatus('unavailable');
            }
        };

        void load();
        return () => {
            active = false;
            unlisten?.();
        };
    }, []);

    return status;
};
