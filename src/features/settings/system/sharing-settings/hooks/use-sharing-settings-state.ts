import { invoke } from '@tauri-apps/api/core';
import { useEffect, useState } from 'react';
import { toast } from 'react-toastify';
import { useTranslation } from '@/i18n';
import { loadSharingSnapshot, updateSharing, type SharingSnapshot } from '../sharing-settings.helpers';

export const useSharingSettingsState = () => {
    const [snapshot, setSnapshot] = useState<SharingSnapshot | null>(null);
    const [loadFailed, setLoadFailed] = useState(false);
    const [isSaving, setIsSaving] = useState(false);
    const [attempt, setAttempt] = useState(0);
    const { t } = useTranslation();

    useEffect(() => {
        let active = true;
        loadSharingSnapshot(invoke)
            .then((value) => {
                if (!active) return;
                setSnapshot(value);
                setLoadFailed(false);
            })
            .catch(() => {
                if (active) setLoadFailed(true);
            });
        return () => {
            active = false;
        };
    }, [attempt]);

    const handleToggle = async (enabled: boolean) => {
        if (snapshot === null || isSaving) return;
        setIsSaving(true);
        try {
            const result = await updateSharing(invoke, enabled, snapshot);
            setSnapshot(result.snapshot);
            setLoadFailed(result.snapshot === null);
            switch (result.error) {
                case 'save':
                    toast.error(t('Failed to save Murmure sharing preference.'));
                    break;
                case 'start':
                    toast.error(t('Failed to enable Murmure sharing.'));
                    break;
                case 'stop':
                    toast.error(t('Failed to disable Murmure sharing.'));
                    break;
                case 'status':
                    toast.error(t('Unable to confirm Murmure sharing status.'));
                    break;
            }
        } finally {
            setIsSaving(false);
        }
    };

    return {
        snapshot,
        loadFailed,
        isSaving,
        handleToggle,
        retry: () => {
            setLoadFailed(false);
            setSnapshot(null);
            setAttempt((value) => value + 1);
        },
    };
};
