import { invoke } from '@tauri-apps/api/core';
import { useEffect, useState } from 'react';
import { Button } from '@/components/button';
import { useIsWayland } from '@/components/hooks/use-linux-session-type';
import { SettingsUI } from '@/components/settings-ui';
import { Typography } from '@/components/typography';
import { ShortcutSettings } from '@/features/settings/settings.types';
import { SHORTCUT_CONFIGS, useShortcut } from '@/features/settings/shortcuts/hooks/use-shortcut';
import { CliCommandRow } from '@/features/settings/shortcuts/shortcuts-cli/cli-commands-panel/cli-command-row/cli-command-row';
import { ShortcutButton } from '@/features/settings/shortcuts/shortcuts-regular/shortcut-button/shortcut-button';
import { useTranslation } from '@/i18n';

const PROMPT_SHORTCUT_FIELDS = [
    ['llm_mode_1_shortcut', 'llm_transform_1_shortcut', 'Prompt 1'],
    ['llm_mode_2_shortcut', 'llm_transform_2_shortcut', 'Prompt 2'],
    ['llm_mode_3_shortcut', 'llm_transform_3_shortcut', 'Prompt 3'],
    ['llm_mode_4_shortcut', 'llm_transform_4_shortcut', 'Prompt 4'],
] as const;

export const RecordShortcutSettings = () => {
    const { t } = useTranslation();
    const isWayland = useIsWayland();
    const { shortcut, setShortcut, resetShortcut } = useShortcut(SHORTCUT_CONFIGS.record);
    const [settings, setSettings] = useState<ShortcutSettings | null>(null);
    const [loadFailed, setLoadFailed] = useState(false);
    const [loadAttempt, setLoadAttempt] = useState(0);

    useEffect(() => {
        if (isWayland) return;
        let active = true;
        setLoadFailed(false);
        invoke<ShortcutSettings>('get_all_settings')
            .then((value) => {
                if (active) setSettings(value);
            })
            .catch(() => {
                if (active) setLoadFailed(true);
            });
        return () => {
            active = false;
        };
    }, [isWayland, loadAttempt]);

    if (isWayland) {
        return (
            <div className="space-y-2" data-testid="record-shortcut-system-managed">
                <SettingsUI.Item>
                    <SettingsUI.Description>
                        <Typography.Title>{t('Shortcuts are managed by your system')}</Typography.Title>
                        <Typography.Paragraph>
                            {t(
                                'Shortcuts are managed by your operating system. Bind these commands in your keyboard settings.'
                            )}
                        </Typography.Paragraph>
                    </SettingsUI.Description>
                </SettingsUI.Item>
                <CliCommandRow label="Toggle transcription" command="murmure --transcription" />
            </div>
        );
    }

    // Hidden shortcuts remain active: keep the native duplicate checks for them.
    const existingShortcuts =
        settings == null
            ? []
            : [
                  { name: t('Paste last transcript'), value: settings.last_transcript_shortcut },
                  { name: t('Cancel recording'), value: settings.cancel_shortcut },
                  { name: t('Toggle Voice Mode'), value: settings.voice_mode_toggle_shortcut },
                  { name: t('Command, free prompt'), value: settings.command_shortcut },
                  ...PROMPT_SHORTCUT_FIELDS.flatMap(([dictateField, transformField, mode]) => [
                      { name: t('Dictate with {{mode}}', { mode }), value: settings[dictateField] },
                      { name: t('Transform with {{mode}}', { mode }), value: settings[transformField] },
                  ]),
              ];

    return (
        <SettingsUI.Item>
            <SettingsUI.Description className="flex-1 min-w-0">
                <Typography.Title>{t('Recording shortcut')}</Typography.Title>
            </SettingsUI.Description>
            {loadFailed ? (
                <div className="flex items-center gap-2" role="alert">
                    <span className="text-sm text-destructive">{t('Failed to load shortcuts')}</span>
                    <Button variant="ghost" onClick={() => setLoadAttempt((attempt) => attempt + 1)}>
                        {t('Retry')}
                    </Button>
                </div>
            ) : settings == null ? (
                <span className="text-sm text-muted-foreground" role="status">
                    {t('Loading...')}
                </span>
            ) : (
                <ShortcutButton
                    keyName={t('Recording shortcut')}
                    shortcut={shortcut}
                    saveShortcut={setShortcut}
                    resetShortcut={resetShortcut}
                    existingShortcuts={existingShortcuts}
                    dataTestId="record-shortcut-button"
                />
            )}
        </SettingsUI.Item>
    );
};
