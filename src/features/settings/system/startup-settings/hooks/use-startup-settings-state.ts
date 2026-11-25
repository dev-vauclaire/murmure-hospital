import { invoke } from '@tauri-apps/api/core';
import { useEffect, useState } from 'react';
import { toast } from 'react-toastify';
import { useTranslation } from '@/i18n';

export const useStartupSettingsState = () => {
    const [startOnLogin, setStartOnLogin] = useState<boolean | null>(null);
    const [loadFailed, setLoadFailed] = useState(false);
    const [isSaving, setIsSaving] = useState(false);
    const [attempt, setAttempt] = useState(0);
    const { t } = useTranslation();

    useEffect(() => {
        let active = true;
        invoke<unknown>('get_start_on_login')
            .then((value) => {
                if (!active) return;
                if (value === null || typeof value === 'boolean') {
                    setStartOnLogin(value);
                    setLoadFailed(false);
                } else {
                    setLoadFailed(true);
                }
            })
            .catch(() => {
                if (active) setLoadFailed(true);
            });
        return () => {
            active = false;
        };
    }, [attempt]);

    const handleToggle = async (enabled: boolean) => {
        if (startOnLogin === null || isSaving) return;
        setIsSaving(true);
        try {
            await invoke('set_start_on_login', { enabled });
            setStartOnLogin(enabled);
        } catch {
            toast.error(t('Failed to save startup preference.'));
        } finally {
            setIsSaving(false);
        }
    };

    return {
        startOnLogin,
        loadFailed,
        isSaving,
        handleToggle,
        retry: () => {
            setLoadFailed(false);
            setAttempt((value) => value + 1);
        },
    };
};
