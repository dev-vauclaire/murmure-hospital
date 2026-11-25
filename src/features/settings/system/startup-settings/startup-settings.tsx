import { Power } from 'lucide-react';
import { Button } from '@/components/button';
import { SettingsUI } from '@/components/settings-ui';
import { Switch } from '@/components/switch';
import { Typography } from '@/components/typography';
import { useTranslation } from '@/i18n';
import { useStartupSettingsState } from './hooks/use-startup-settings-state';

export const StartupSettings = () => {
    const { t } = useTranslation();
    const { startOnLogin, loadFailed, isSaving, handleToggle, retry } = useStartupSettingsState();

    if (loadFailed) {
        return (
            <>
                <SettingsUI.Item>
                    <div className="flex items-center gap-2" role="alert">
                        <Typography.Paragraph>{t('Unable to load startup preference.')}</Typography.Paragraph>
                        <Button variant="ghost" onClick={retry}>
                            {t('Retry')}
                        </Button>
                    </div>
                </SettingsUI.Item>
                <SettingsUI.Separator />
            </>
        );
    }

    // The backend returns null outside Windows; keep the setting hidden until loaded.
    if (startOnLogin === null) return null;

    return (
        <>
            <SettingsUI.Item>
                <SettingsUI.Description>
                    <Typography.Title>
                        <label htmlFor="start-on-login" className="flex items-center gap-2">
                            <Power className="w-4 h-4 text-muted-foreground" aria-hidden="true" />
                            {t('Start Murmure when I sign in')}
                        </label>
                    </Typography.Title>
                    <Typography.Paragraph id="start-on-login-description">
                        {t('Applies only to your Windows account, starting with your next sign-in.')}
                    </Typography.Paragraph>
                </SettingsUI.Description>
                <Switch
                    id="start-on-login"
                    data-testid="start-on-login-switch"
                    aria-describedby="start-on-login-description"
                    checked={startOnLogin}
                    disabled={isSaving}
                    onCheckedChange={handleToggle}
                />
            </SettingsUI.Item>
            <SettingsUI.Separator />
        </>
    );
};
