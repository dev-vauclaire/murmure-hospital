import { Mic } from 'lucide-react';
import { SettingsUI } from '@/components/settings-ui';
import { Typography } from '@/components/typography';
import { useTranslation } from '@/i18n';
import { useSpeechMikeStatus, type SpeechMikeStatusValue } from './hooks/use-speechmike-status';

const STATUS_LABELS: Record<Exclude<SpeechMikeStatusValue, 'unsupported'>, string> = {
    searching: 'No SpeechMike detected',
    initializing: 'Press and release REC to initialize the SpeechMike buttons',
    ready: 'SpeechMike ready',
    disconnected: 'SpeechMike disconnected',
    multiple_devices: 'Connect only one SpeechMike',
    wrong_mode: 'Select Event mode on the SpeechMike',
    unsupported_device: 'Unsupported SpeechMike model',
    unavailable: 'SpeechMike unavailable',
};

export const SpeechMikeStatus = () => {
    const { t } = useTranslation();
    const status = useSpeechMikeStatus();

    if (status == null || status === 'unsupported') return null;

    const statusColor = status === 'ready' ? 'text-emerald-400' : 'text-muted-foreground';

    return (
        <>
            <SettingsUI.Separator />
            <SettingsUI.Item>
                <SettingsUI.Description className="flex-1 min-w-0">
                    <Typography.Title className="flex items-center gap-2">
                        <Mic className="w-4 h-4 text-muted-foreground" />
                        SpeechMike SMP3700
                    </Typography.Title>
                    <Typography.Paragraph>
                        {t('REC: dictate · F1: cancel · F2: paste last transcription.')}
                    </Typography.Paragraph>
                    <Typography.Paragraph>{t('Uses the microphone selected in Murmure.')}</Typography.Paragraph>
                </SettingsUI.Description>
                <span
                    role="status"
                    aria-live="polite"
                    className={`max-w-[240px] text-right text-sm ${statusColor}`}
                    data-testid="speechmike-status"
                >
                    {t(STATUS_LABELS[status])}
                </span>
            </SettingsUI.Item>
        </>
    );
};
