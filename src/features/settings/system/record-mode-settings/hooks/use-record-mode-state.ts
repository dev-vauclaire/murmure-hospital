import { invoke } from '@tauri-apps/api/core';
import { useEffect, useRef, useState } from 'react';
import { toast } from 'react-toastify';
import { useTranslation } from '@/i18n';
import { AppSettings } from '@/features/settings/settings.types';

export type RecordMode = 'push_to_talk' | 'toggle_to_talk';

const UPDATED_EVENT = 'murmure-record-mode-updated';
const isRecordMode = (value: unknown): value is RecordMode => value === 'push_to_talk' || value === 'toggle_to_talk';

export const useRecordModeState = () => {
    const [recordMode, setRecordMode] = useState<RecordMode>('push_to_talk');
    const [isLoaded, setIsLoaded] = useState(false);
    const [loadFailed, setLoadFailed] = useState(false);
    const [isSaving, setIsSaving] = useState(false);
    const [attempt, setAttempt] = useState(0);
    const mounted = useRef(false);
    const saving = useRef(false);
    const { t } = useTranslation();

    useEffect(() => {
        let active = true;
        let updated = false;
        mounted.current = true;
        const handleUpdated = (event: Event) => {
            const { detail } = event as CustomEvent<unknown>;
            if (!isRecordMode(detail)) return;
            updated = true;
            setRecordMode(detail);
            setIsLoaded(true);
            setLoadFailed(false);
        };
        window.addEventListener(UPDATED_EVENT, handleUpdated);
        invoke<AppSettings>('get_all_settings')
            .then((settings) => {
                if (!active || updated) return;
                if (isRecordMode(settings.record_mode)) {
                    setRecordMode(settings.record_mode);
                    setIsLoaded(true);
                    setLoadFailed(false);
                } else {
                    setLoadFailed(true);
                }
            })
            .catch(() => {
                if (active && !updated) setLoadFailed(true);
            });
        return () => {
            active = false;
            mounted.current = false;
            window.removeEventListener(UPDATED_EVENT, handleUpdated);
        };
    }, [attempt]);

    return {
        recordMode,
        isLoading: !isLoaded && !loadFailed,
        loadFailed,
        isSaving,
        retry: () => {
            setLoadFailed(false);
            setAttempt((value) => value + 1);
        },
        setRecordMode: async (mode: RecordMode) => {
            if (saving.current) return;
            saving.current = true;
            setIsSaving(true);
            try {
                await invoke('set_record_mode', { mode });
                // Only a persisted change updates the selector and all REC hints.
                window.dispatchEvent(new CustomEvent(UPDATED_EVENT, { detail: mode }));
            } catch {
                toast.error(t('Failed to save record mode'));
            } finally {
                saving.current = false;
                if (mounted.current) setIsSaving(false);
            }
        },
    };
};
