import { Share2 } from 'lucide-react';
import { Button } from '@/components/button';
import { SettingsUI } from '@/components/settings-ui';
import { Switch } from '@/components/switch';
import { Typography } from '@/components/typography';
import { useTranslation } from '@/i18n';
import { useSharingSettingsState } from './hooks/use-sharing-settings-state';

export const SharingSettings = () => {
    const { t } = useTranslation();
    const { snapshot, loadFailed, isSaving, handleToggle, retry } = useSharingSettingsState();

    return (
        <>
            <SettingsUI.Item>
                <SettingsUI.Description>
                    <Typography.Title>
                        <label htmlFor="murmure-sharing" className="flex items-center gap-2">
                            <Share2 className="w-4 h-4 text-muted-foreground" aria-hidden="true" />
                            {t('Murmure sharing')}
                        </label>
                    </Typography.Title>
                    <Typography.Paragraph id="murmure-sharing-description">
                        {t('Allow other applications on this computer to send audio to Murmure for transcription.')}
                    </Typography.Paragraph>
                    {loadFailed && (
                        <div className="flex items-center gap-2" role="alert">
                            <Typography.Paragraph>
                                {t('Unable to confirm Murmure sharing status.')}
                            </Typography.Paragraph>
                            <Button variant="ghost" onClick={retry}>
                                {t('Retry')}
                            </Button>
                        </div>
                    )}
                    {snapshot !== null && snapshot.enabled && snapshot.status !== 'running' && (
                        <Typography.Paragraph role="status">
                            {t('Murmure sharing is unavailable. Turn it off and on again to retry.')}
                        </Typography.Paragraph>
                    )}
                </SettingsUI.Description>
                {!loadFailed && (
                    <div className="flex items-center gap-2" aria-busy={snapshot === null || isSaving}>
                        {snapshot === null && <span role="status">{t('Loading...')}</span>}
                        <Switch
                            id="murmure-sharing"
                            data-testid="murmure-sharing-switch"
                            aria-describedby="murmure-sharing-description"
                            checked={snapshot?.enabled ?? false}
                            disabled={snapshot === null || isSaving}
                            onCheckedChange={handleToggle}
                        />
                    </div>
                )}
            </SettingsUI.Item>
            <SettingsUI.Separator />
        </>
    );
};
