export type MicrophoneControlProfile = 'keyboard' | 'speechmike';

export type SpeechMikeStatus =
    | 'unsupported'
    | 'searching'
    | 'initializing'
    | 'ready'
    | 'disconnected'
    | 'multiple_devices'
    | 'wrong_mode'
    | 'unsupported_device'
    | 'unavailable';

export interface MicrophoneControlsSnapshot {
    profile: MicrophoneControlProfile;
    pendingProfile: MicrophoneControlProfile | null;
    speechMikeStatus: SpeechMikeStatus;
    identityStatus: 'matched' | 'not_matched' | 'unavailable' | 'unsupported';
}

export interface MicrophoneControlsState {
    controls: MicrophoneControlsSnapshot | null;
    loadFailed: boolean;
    retry: () => void;
}
