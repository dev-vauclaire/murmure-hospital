import { useEffect, useState, useCallback, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { toast } from 'react-toastify';
import { useTranslation } from '@/i18n';
import { FormattingSettings, FormattingRule, MatchMode, defaultFormattingSettings, migrateRule } from '../types';
import {
    deleteHospitalRule,
    reconcileHospitalRuleHelpers,
    reorderVisibleHospitalRules,
    updateHospitalRule,
} from '../hospital-rule-settings.helpers';

export const useFormattingRules = () => {
    const [settings, setSettings] = useState<FormattingSettings>(defaultFormattingSettings);
    const [isLoading, setIsLoading] = useState(true);
    const [loadError, setLoadError] = useState(false);
    const loadRequest = useRef(0);
    const { t } = useTranslation();

    const loadSettings = useCallback(async () => {
        const request = ++loadRequest.current;
        setIsLoading(true);
        setLoadError(false);
        try {
            const loaded = await invoke<FormattingSettings>('get_formatting_settings');
            if (request !== loadRequest.current) return;
            const migratedSettings = {
                ...loaded,
                rules: loaded.rules.map((rule) => migrateRule(rule as unknown as Record<string, unknown>)),
            };
            const reconciledRules = reconcileHospitalRuleHelpers(migratedSettings.rules);
            const reconciledSettings = { ...migratedSettings, rules: reconciledRules };
            // Persist before exposing the editor: the backend must use the same
            // enabled state as the visible command when its helper is hidden.
            if (reconciledRules !== migratedSettings.rules) {
                await invoke('set_formatting_settings', { settings: reconciledSettings });
            }
            if (request !== loadRequest.current) return;
            setSettings(reconciledSettings);
        } catch {
            if (request !== loadRequest.current) return;
            console.error('Failed to load formatting settings');
            setLoadError(true);
        } finally {
            if (request === loadRequest.current) setIsLoading(false);
        }
    }, []);

    useEffect(() => {
        loadSettings();
        // Ignore stale loads, including the first StrictMode effect in development.
        return () => {
            loadRequest.current++;
        };
    }, [loadSettings]);

    const saveSettings = useCallback(
        async (newSettings: FormattingSettings) => {
            setSettings(newSettings);
            try {
                await invoke('set_formatting_settings', {
                    settings: newSettings,
                });
            } catch (error) {
                console.error('Failed to save formatting settings:', error);
                toast.error(t('Failed to save formatting rules'));
            }
        },
        [t]
    );

    const updateBuiltInOption = useCallback(
        async (key: keyof FormattingSettings['built_in'], value: boolean | string | number) => {
            const newSettings = {
                ...settings,
                built_in: {
                    ...settings.built_in,
                    [key]: value,
                },
            };
            await saveSettings(newSettings);
        },
        [settings, saveSettings]
    );

    const addRule = useCallback(
        async (trigger: string, replacement: string, matchMode: MatchMode) => {
            const newRule: FormattingRule = {
                id: crypto.randomUUID(),
                trigger,
                replacement,
                enabled: true,
                match_mode: matchMode,
            };
            const newSettings = {
                ...settings,
                rules: [...settings.rules, newRule],
            };
            await saveSettings(newSettings);
        },
        [settings, saveSettings]
    );

    const updateRule = useCallback(
        async (id: string, updates: Partial<Omit<FormattingRule, 'id'>>) => {
            const newSettings = {
                ...settings,
                rules: updateHospitalRule(settings.rules, id, updates),
            };
            await saveSettings(newSettings);
        },
        [settings, saveSettings]
    );

    const deleteRule = useCallback(
        async (id: string) => {
            const newSettings = {
                ...settings,
                rules: deleteHospitalRule(settings.rules, id),
            };
            await saveSettings(newSettings);
        },
        [settings, saveSettings]
    );

    const duplicateRule = useCallback(
        async (id: string) => {
            const ruleToDuplicate = settings.rules.find((rule) => rule.id === id);
            if (!ruleToDuplicate) return;

            const newRule: FormattingRule = {
                ...ruleToDuplicate,
                id: crypto.randomUUID(),
            };
            const newSettings = {
                ...settings,
                rules: [...settings.rules, newRule],
            };
            await saveSettings(newSettings);
        },
        [settings, saveSettings]
    );

    const reorderRules = useCallback(
        async (activeId: string, overId: string) => {
            const newRules = reorderVisibleHospitalRules(settings.rules, activeId, overId);
            if (newRules === settings.rules) return;

            const newSettings = {
                ...settings,
                rules: newRules,
            };
            await saveSettings(newSettings);
        },
        [settings, saveSettings]
    );

    return {
        settings,
        isLoading,
        loadError,
        retryLoad: loadSettings,
        updateBuiltInOption,
        addRule,
        updateRule,
        deleteRule,
        duplicateRule,
        reorderRules,
    };
};
