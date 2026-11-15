import { useState } from 'react';
import { Typography } from '@/components/typography';
import { useTranslation } from '@/i18n';
import { useRecordModeState } from '../../record-mode-settings/hooks/use-record-mode-state';
import { getSpeechMikeStatusLabel } from '../microphone-controls.helpers';
import type { SpeechMikeStatus } from '../microphone-controls.types';

const GUIDE_IMAGES = import.meta.glob<string>('/src/assets/speechmike/smp3700-guide.{png,jpg,jpeg,webp,svg}', {
    eager: true,
    query: '?url',
    import: 'default',
});
const GUIDE_IMAGE = Object.values(GUIDE_IMAGES)[0] ?? null;

export const SpeechMikeGuide = ({ status }: { status: SpeechMikeStatus }) => {
    const { t } = useTranslation();
    const { recordMode, isLoading, loadFailed } = useRecordModeState();
    const [imageFailed, setImageFailed] = useState(false);

    const actions = [
        {
            button: 'REC',
            action: t('Dictate'),
            description:
                isLoading || loadFailed
                    ? t('Dictate')
                    : recordMode === 'push_to_talk'
                      ? t('Hold REC to record, release to transcribe.')
                      : t('Press REC to start, then press it again to stop.'),
        },
        { button: 'F1', action: t('Cancel recording'), description: t('Cancels the current recording.') },
        { button: 'F2', action: t('Paste last transcript'), description: t('Pastes the last transcript.') },
    ];

    return (
        <section className="space-y-4 px-4 py-4" aria-label={t('SpeechMike controls')} data-testid="speechmike-guide">
            <div className="flex flex-wrap items-center justify-between gap-2">
                <Typography.Title>SpeechMike SMP3700</Typography.Title>
                <span
                    role="status"
                    aria-live="polite"
                    className={`text-sm ${status === 'ready' ? 'text-emerald-400' : 'text-muted-foreground'}`}
                    data-testid="speechmike-status"
                >
                    {t(getSpeechMikeStatusLabel(status))}
                </span>
            </div>

            {GUIDE_IMAGE !== null && !imageFailed && (
                <img
                    src={GUIDE_IMAGE}
                    alt={t('SpeechMike SMP3700 with the REC, F1 and F2 buttons identified.')}
                    className="mx-auto max-h-[440px] max-w-full rounded-md object-contain"
                    onError={() => setImageFailed(true)}
                />
            )}

            <dl className="grid gap-3 sm:grid-cols-3">
                {actions.map(({ button, action, description }) => (
                    <div
                        key={button}
                        className="space-y-2 rounded-md border border-surface-border bg-background/50 p-3"
                    >
                        <dt className="flex items-center gap-2 text-sm font-semibold">
                            <span className="rounded border border-input bg-field px-2 py-0.5 font-mono text-sky-300">
                                {button}
                            </span>
                            {action}
                        </dt>
                        <dd className="text-xs text-muted-foreground">{description}</dd>
                    </div>
                ))}
            </dl>
            <Typography.Paragraph className="text-xs">
                {t(
                    'Use the physical buttons on the SpeechMike. Your keyboard shortcuts are kept and temporarily disabled.'
                )}
            </Typography.Paragraph>
        </section>
    );
};
