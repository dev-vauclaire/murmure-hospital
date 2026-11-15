import React from 'react';
import ReactDOM from 'react-dom/client';
import { RouterProvider } from '@tanstack/react-router';
import { router } from './router';
import './tailwind.css';
import { MicrophoneControlsProvider } from './features/settings/system/microphone-controls/microphone-controls-provider';

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
    <React.StrictMode>
        <MicrophoneControlsProvider>
            <RouterProvider router={router} />
        </MicrophoneControlsProvider>
    </React.StrictMode>
);
