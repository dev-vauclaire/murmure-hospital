import { Typography } from '@/components/typography';
import { SettingsUI } from '@/components/settings-ui';
import { Page } from '@/components/page';
import { PasteMethodSettings } from './paste-method-settings/paste-method-settings';
import { SoundSettings } from './sound-settings/sound-settings';
import { MicSettings } from './mic-settings/mic-settings';
import { RecordModeSettings } from './record-mode-settings/record-mode-settings';
import { useTranslation } from '@/i18n';
import { Settings } from 'lucide-react';

export const System = () => {
    const { t } = useTranslation();
    return (
        <main>
            <div className="space-y-8">
                <Page.Header>
                    <Typography.MainTitle data-testid="system-title">{t('System')}</Typography.MainTitle>
                </Page.Header>

                <div className="flex justify-center">
                    <div className="w-full space-y-6">
                        <SettingsUI.Section title={t('General')} icon={Settings}>
                            <MicSettings />
                            <SettingsUI.Separator />
                            <RecordModeSettings />
                            <SettingsUI.Separator />
                            <SoundSettings />
                            <SettingsUI.Separator />
                            <PasteMethodSettings />
                        </SettingsUI.Section>
                    </div>
                </div>
            </div>
        </main>
    );
};
