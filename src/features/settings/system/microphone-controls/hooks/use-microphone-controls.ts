import { useContext } from 'react';
import { MicrophoneControlsContext } from '../microphone-controls-context';

export const useMicrophoneControls = () => {
    const context = useContext(MicrophoneControlsContext);
    if (context === null) throw new Error('Microphone controls require their provider.');
    return context;
};
