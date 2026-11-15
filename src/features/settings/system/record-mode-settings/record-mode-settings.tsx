import { SettingsUI } from '@/components/settings-ui';
import { Typography } from '@/components/typography';
import { Button } from '@/components/button';
import { Mic } from 'lucide-react';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/select';
import { useTranslation } from '@/i18n';
import { useIsWayland } from '@/components/hooks/use-linux-session-type';
import {
    RecordMode,
    useRecordModeState,
} from '@/features/settings/system/record-mode-settings/hooks/use-record-mode-state.ts';

const SUPPORTED_RECORD_MODE: { key: RecordMode; label: string }[] = [
    { key: 'push_to_talk', label: 'Push to talk' },
    { key: 'toggle_to_talk', label: 'Toggle to talk' },
];

export const RecordModeSettings = () => {
    const { t } = useTranslation();
    const { recordMode, setRecordMode, isLoading, loadFailed, isSaving, retry } = useRecordModeState();
    const isWayland = useIsWayland();

    return (
        <SettingsUI.Item>
            <SettingsUI.Description>
                <Typography.Title className="flex items-center gap-2">
                    <Mic className="w-4 h-4 text-muted-foreground" />
                    {t('Record mode')}
                </Typography.Title>
                <Typography.Paragraph>{t('Choose how recording is triggered.')}</Typography.Paragraph>
            </SettingsUI.Description>
            {loadFailed ? (
                <div className="flex items-center gap-2" role="alert">
                    <span className="text-sm text-destructive">{t('Failed to load record mode')}</span>
                    <Button variant="ghost" onClick={retry}>
                        {t('Retry')}
                    </Button>
                </div>
            ) : (
                <Select value={recordMode} onValueChange={setRecordMode} disabled={isLoading || isSaving}>
                    <SelectTrigger className="w-[260px]" data-testid="record-mode-select">
                        <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                        {SUPPORTED_RECORD_MODE.map((mode) => {
                            const isPushToTalkDisabled = isWayland && mode.key === 'push_to_talk';
                            return (
                                <SelectItem key={mode.key} value={mode.key} disabled={isPushToTalkDisabled}>
                                    {t(mode.label)}
                                    {isPushToTalkDisabled && (
                                        <span className="ml-2 text-xs text-muted-foreground">
                                            {t('Not supported on Wayland')}
                                        </span>
                                    )}
                                </SelectItem>
                            );
                        })}
                    </SelectContent>
                </Select>
            )}
        </SettingsUI.Item>
    );
};
