export type SharingServerState = 'stopped' | 'running' | 'port_in_use' | 'failed';

export interface SharingSnapshot {
    enabled: boolean;
    status: SharingServerState;
}

export interface SharingCommands {
    <T>(command: string, args?: Record<string, unknown>): Promise<T>;
}

export type SharingError = 'save' | 'start' | 'stop' | 'status';

export interface SharingUpdate {
    snapshot: SharingSnapshot | null;
    error: SharingError | null;
}

const loadServerState = async (invoke: SharingCommands): Promise<SharingServerState> => {
    const status = await invoke<unknown>('get_http_api_status');
    if (status !== null && typeof status === 'object' && 'state' in status) {
        switch (status.state) {
            case 'stopped':
            case 'running':
            case 'port_in_use':
            case 'failed':
                return status.state;
        }
    }
    throw new Error('Invalid sharing status');
};

export const loadSharingSnapshot = async (invoke: SharingCommands): Promise<SharingSnapshot> => {
    const [enabled, status] = await Promise.all([invoke<unknown>('get_api_enabled'), loadServerState(invoke)]);
    if (typeof enabled !== 'boolean') throw new Error('Invalid sharing preference');
    // Preserve the upstream startup preference after a boot error, while never
    // presenting a running server as disabled if saving its preference failed.
    return { enabled: enabled || status === 'running', status };
};

export const updateSharing = async (
    invoke: SharingCommands,
    enabled: boolean,
    previous: SharingSnapshot
): Promise<SharingUpdate> => {
    // Keep the upstream order: persist the choice before changing the server.
    try {
        await invoke('set_api_enabled', { enabled });
    } catch {
        return { snapshot: previous, error: 'save' };
    }

    let actionFailed = false;
    try {
        await invoke(enabled ? 'start_http_api_server' : 'stop_http_api_server');
    } catch {
        actionFailed = true;
    }

    let status: SharingServerState;
    try {
        status = await loadServerState(invoke);
    } catch {
        // A failed status read does not prove that start/stop failed. Leave the
        // saved choice intact and ask the UI to reload instead of guessing.
        return { snapshot: null, error: 'status' };
    }

    const running = status === 'running';
    if (actionFailed || running !== enabled) {
        if (running !== enabled) {
            try {
                await invoke('set_api_enabled', { enabled: running });
            } catch {
                // The compensating write may have failed before or after being
                // applied. Read both sources again rather than claiming success.
                try {
                    return { snapshot: await loadSharingSnapshot(invoke), error: 'save' };
                } catch {
                    return { snapshot: null, error: 'status' };
                }
            }
        }
        return { snapshot: { enabled: running, status }, error: enabled ? 'start' : 'stop' };
    }

    return { snapshot: { enabled, status }, error: null };
};
