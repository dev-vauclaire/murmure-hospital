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

export const KeyboardShortcutSettings = () => {
    const { t } = useTranslation();
    const isWayland = useIsWayland();
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
                <SettingsUI.Separator />
                <CliCommandRow label="Paste last transcript" command="murmure --paste-last" />
                <SettingsUI.Separator />
                <CliCommandRow label="Cancel recording" command="murmure --cancel" />
            </div>
        );
    }

    if (loadFailed) {
        return (
            <SettingsUI.Item>
                <div className="flex items-center gap-2" role="alert">
                    <span className="text-sm text-destructive">{t('Failed to load shortcuts')}</span>
                    <Button variant="ghost" onClick={() => setLoadAttempt((attempt) => attempt + 1)}>
                        {t('Retry')}
                    </Button>
                </div>
            </SettingsUI.Item>
        );
    }
    if (settings === null) {
        return (
            <SettingsUI.Item>
                <span className="text-sm text-muted-foreground" role="status">
                    {t('Loading...')}
                </span>
            </SettingsUI.Item>
        );
    }
    return <KeyboardShortcutBindings initialSettings={settings} />;
};

const KeyboardShortcutBindings = ({ initialSettings }: { initialSettings: ShortcutSettings }) => {
    const { t } = useTranslation();
    const record = useShortcut({ ...SHORTCUT_CONFIGS.record, initialShortcut: initialSettings.record_shortcut });
    const pasteLast = useShortcut({
        ...SHORTCUT_CONFIGS.lastTranscript,
        initialShortcut: initialSettings.last_transcript_shortcut,
    });
    const cancel = useShortcut({ ...SHORTCUT_CONFIGS.cancel, initialShortcut: initialSettings.cancel_shortcut });

    const shortcuts = [
        { id: 'record', title: t('Recording shortcut'), testId: 'record-shortcut-button', ...record },
        { id: 'pasteLast', title: t('Paste last transcript'), testId: 'paste-last-shortcut-button', ...pasteLast },
        { id: 'cancel', title: t('Cancel recording'), testId: 'cancel-shortcut-button', ...cancel },
    ];
    // A legacy profile may still contain hidden bindings. Include them in the
    // same conflict checks as the three editable shortcuts.
    const otherShortcuts = [
        { name: t('Toggle Voice Mode'), value: initialSettings.voice_mode_toggle_shortcut },
        { name: t('Command, free prompt'), value: initialSettings.command_shortcut },
        ...PROMPT_SHORTCUT_FIELDS.flatMap(([dictateField, transformField, mode]) => [
            { name: t('Dictate with {{mode}}', { mode }), value: initialSettings[dictateField] },
            { name: t('Transform with {{mode}}', { mode }), value: initialSettings[transformField] },
        ]),
    ];

    return (
        <div data-testid="keyboard-microphone-controls">
            {shortcuts.map(({ id, title, testId, shortcut, setShortcut, resetShortcut }, index) => (
                <div key={id}>
                    {index > 0 && <SettingsUI.Separator />}
                    <SettingsUI.Item>
                        <SettingsUI.Description className="flex-1 min-w-0">
                            <Typography.Title>{title}</Typography.Title>
                        </SettingsUI.Description>
                        <ShortcutButton
                            keyName={title}
                            shortcut={shortcut}
                            saveShortcut={setShortcut}
                            resetShortcut={resetShortcut}
                            existingShortcuts={[
                                ...shortcuts
                                    .filter((item) => item.id !== id)
                                    .map((item) => ({ name: item.title, value: item.shortcut })),
                                ...otherShortcuts,
                            ]}
                            dataTestId={testId}
                        />
                    </SettingsUI.Item>
                </div>
            ))}
        </div>
    );
};
