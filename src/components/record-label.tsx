import { RenderKeys } from '@/components/render-keys.tsx';
import { Typography } from '@/components/typography.tsx';
import { useTranslation } from '@/i18n';
import { useRecordModeState } from '@/features/settings/system/record-mode-settings/hooks/use-record-mode-state.ts';
import { useShortcut, SHORTCUT_CONFIGS } from '@/features/settings/shortcuts/hooks/use-shortcut.ts';
import { useIsWayland } from '@/components/hooks/use-linux-session-type';
import { useMicrophoneControls } from '@/features/settings/system/microphone-controls/hooks/use-microphone-controls';
import { getSpeechMikeStatusLabel } from '@/features/settings/system/microphone-controls/microphone-controls.helpers';

export const RecordLabel = () => {
    const isWayland = useIsWayland();

    const { recordMode, isLoading: recordModeLoading, loadFailed: recordModeFailed } = useRecordModeState();
    const { shortcut: recordShortcut } = useShortcut(SHORTCUT_CONFIGS.record);
    const { controls, loadFailed } = useMicrophoneControls();
    const { t } = useTranslation();

    if (isWayland || controls === null || loadFailed || recordModeLoading || recordModeFailed) {
        return null;
    }

    if (controls.profile === 'speechmike') {
        return (
            <Typography.Paragraph className="text-xs absolute bottom-2 left-2" aria-live="polite">
                {controls.speechMikeStatus !== 'ready'
                    ? t(getSpeechMikeStatusLabel(controls.speechMikeStatus))
                    : recordMode === 'push_to_talk'
                      ? t('Hold REC to record, release to transcribe.')
                      : t('Press REC to start, then press it again to stop.')}
            </Typography.Paragraph>
        );
    }

    if (recordShortcut.length === 0) return null;

    return (
        <Typography.Paragraph className="text-xs absolute bottom-2 left-2">
            {recordMode === 'push_to_talk' ? (
                <>
                    {t('Hold')} <RenderKeys keyString={recordShortcut} />
                    {t(' to record')}
                </>
            ) : (
                <>
                    {t('Toggle')} <RenderKeys keyString={recordShortcut} />
                    {t(' to start/stop recording')}
                </>
            )}
        </Typography.Paragraph>
    );
};
