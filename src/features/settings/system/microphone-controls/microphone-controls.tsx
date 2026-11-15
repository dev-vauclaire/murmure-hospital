import { Button } from '@/components/button';
import { SettingsUI } from '@/components/settings-ui';
import { Typography } from '@/components/typography';
import { useTranslation } from '@/i18n';
import { KeyboardShortcutSettings } from '../keyboard-shortcut-settings/keyboard-shortcut-settings';
import { useMicrophoneControls } from './hooks/use-microphone-controls';
import { SpeechMikeGuide } from './speechmike-guide/speechmike-guide';

export const MicrophoneControls = () => {
    const { t } = useTranslation();
    const { controls, loadFailed, retry } = useMicrophoneControls();

    if (loadFailed) {
        return (
            <SettingsUI.Item>
                <div className="flex items-center gap-2" role="alert">
                    <Typography.Paragraph>{t('Unable to load microphone controls.')}</Typography.Paragraph>
                    <Button variant="ghost" onClick={retry}>
                        {t('Retry')}
                    </Button>
                </div>
            </SettingsUI.Item>
        );
    }
    if (controls === null) {
        return (
            <SettingsUI.Item>
                <span className="text-sm text-muted-foreground" role="status">
                    {t('Loading...')}
                </span>
            </SettingsUI.Item>
        );
    }

    return (
        <>
            {controls.pendingProfile !== null && (
                <p role="status" className="px-4 pt-4 text-sm text-sky-300" data-testid="controls-profile-pending">
                    {t('The controls will change after the current dictation finishes.')}
                </p>
            )}
            {controls.profile === 'speechmike' ? (
                <SpeechMikeGuide status={controls.speechMikeStatus} />
            ) : (
                <>
                    {controls.identityStatus === 'unavailable' && (
                        <p role="status" className="px-4 pt-4 text-sm text-muted-foreground">
                            {t('SpeechMike detection is unavailable. Keyboard controls remain active.')}
                        </p>
                    )}
                    <KeyboardShortcutSettings />
                </>
            )}
        </>
    );
};
