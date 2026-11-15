import { createContext } from 'react';
import type { MicrophoneControlsState } from './microphone-controls.types';

export const MicrophoneControlsContext = createContext<MicrophoneControlsState | null>(null);
