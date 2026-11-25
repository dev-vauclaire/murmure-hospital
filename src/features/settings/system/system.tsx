import { Typography } from '@/components/typography';
import { SettingsUI } from '@/components/settings-ui';
import { Page } from '@/components/page';
import { PasteMethodSettings } from './paste-method-settings/paste-method-settings';
import { SoundSettings } from './sound-settings/sound-settings';
import { MicSettings } from './mic-settings/mic-settings';
import { RecordModeSettings } from './record-mode-settings/record-mode-settings';
import { MicrophoneControls } from './microphone-controls/microphone-controls';
import { StartupSettings } from './startup-settings/startup-settings';
import { SharingSettings } from './sharing-settings/sharing-settings';
import { useTranslation } from '@/i18n';
import { Mic, Settings } from 'lucide-react';

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
                        <SettingsUI.Section title={t('Microphone and controls')} icon={Mic}>
                            <MicSettings />
                            <SettingsUI.Separator />
                            <RecordModeSettings />
                            <SettingsUI.Separator />
                            <MicrophoneControls />
                        </SettingsUI.Section>
                        <SettingsUI.Section title={t('General')} icon={Settings}>
                            <StartupSettings />
                            <SharingSettings />
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
