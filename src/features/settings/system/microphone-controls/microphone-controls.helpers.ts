import type {
    MicrophoneControlProfile,
    MicrophoneControlsSnapshot,
    SpeechMikeStatus,
} from './microphone-controls.types';

const SPEECHMIKE_STATUSES: readonly string[] = [
    'unsupported',
    'searching',
    'initializing',
    'ready',
    'disconnected',
    'multiple_devices',
    'wrong_mode',
    'unsupported_device',
    'unavailable',
];

const IDENTITY_STATUSES: readonly string[] = ['matched', 'not_matched', 'unavailable', 'unsupported'];

const isProfile = (value: unknown): value is MicrophoneControlProfile => value === 'keyboard' || value === 'speechmike';

export const parseMicrophoneControls = (value: unknown): MicrophoneControlsSnapshot | null => {
    if (typeof value !== 'object' || value === null) return null;
    const snapshot = value as Record<string, unknown>;
    if (!isProfile(snapshot.profile)) return null;
    if (snapshot.pendingProfile !== null && !isProfile(snapshot.pendingProfile)) return null;
    if (typeof snapshot.speechMikeStatus !== 'string' || !SPEECHMIKE_STATUSES.includes(snapshot.speechMikeStatus)) {
        return null;
    }
    if (typeof snapshot.identityStatus !== 'string' || !IDENTITY_STATUSES.includes(snapshot.identityStatus)) {
        return null;
    }
    return {
        profile: snapshot.profile,
        pendingProfile: snapshot.pendingProfile,
        speechMikeStatus: snapshot.speechMikeStatus as SpeechMikeStatus,
        identityStatus: snapshot.identityStatus as MicrophoneControlsSnapshot['identityStatus'],
    };
};

export const getSpeechMikeStatusLabel = (status: SpeechMikeStatus): string => {
    switch (status) {
        case 'ready':
            return 'SpeechMike ready';
        case 'initializing':
            return 'Press and release REC to initialize the SpeechMike buttons';
        case 'searching':
            return 'No SpeechMike detected';
        case 'disconnected':
            return 'SpeechMike disconnected';
        case 'multiple_devices':
            return 'Connect only one SpeechMike';
        case 'wrong_mode':
            return 'Select Event mode on the SpeechMike';
        case 'unsupported_device':
            return 'Unsupported SpeechMike model';
        default:
            return 'SpeechMike unavailable';
    }
};
