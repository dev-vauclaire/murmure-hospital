import type { ReactNode } from 'react';
import { MicrophoneControlsContext } from './microphone-controls-context';
import { useMicrophoneControlsState } from './hooks/use-microphone-controls-state';

export const MicrophoneControlsProvider = ({ children }: { children: ReactNode }) => {
    const state = useMicrophoneControlsState();
    return <MicrophoneControlsContext.Provider value={state}>{children}</MicrophoneControlsContext.Provider>;
};
