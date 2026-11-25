import assert from 'node:assert/strict';
import test from 'node:test';
import {
    loadSharingSnapshot,
    updateSharing,
} from '../src/features/settings/system/sharing-settings/sharing-settings.helpers.ts';

const scriptedCommands = (steps) => {
    const pending = [...steps];
    const calls = [];
    const invoke = async (command, args) => {
        calls.push({ command, args });
        const step = pending.shift();
        assert.ok(step, `Unexpected command: ${command}`);
        assert.equal(command, step.command);
        assert.deepEqual(args, step.args);
        if (step.error) throw new Error('Synthetic command failure');
        return step.result;
    };
    return {
        invoke,
        done: () => {
            assert.equal(pending.length, 0);
            assert.deepEqual(
                calls,
                steps.map(({ command, args }) => ({ command, args }))
            );
        },
    };
};

const setPreference = (enabled, error = false) => ({ command: 'set_api_enabled', args: { enabled }, error });
const readPreference = (result, error = false) => ({ command: 'get_api_enabled', result, error });
const readStatus = (state, error = false) => ({ command: 'get_http_api_status', result: { state }, error });
const start = (error = false) => ({ command: 'start_http_api_server', error });
const stop = (error = false) => ({ command: 'stop_http_api_server', error });
const stopped = { enabled: false, status: 'stopped' };
const running = { enabled: true, status: 'running' };

test('initial loading reads the native preference and status without starting or saving anything', async () => {
    for (const snapshot of [stopped, running, { enabled: true, status: 'port_in_use' }]) {
        const commands = scriptedCommands([readPreference(snapshot.enabled), readStatus(snapshot.status)]);
        assert.deepEqual(await loadSharingSnapshot(commands.invoke), snapshot);
        commands.done();
    }
});

test('a running server is not presented as stopped when the saved preference differs', async () => {
    const commands = scriptedCommands([readPreference(false), readStatus('running')]);
    assert.deepEqual(await loadSharingSnapshot(commands.invoke), running);
    commands.done();
});

test('invalid native preference or server status cannot silently mean sharing is disabled', async () => {
    for (const [preference, status] of [
        [null, 'stopped'],
        [false, 'unknown'],
        [true, undefined],
    ]) {
        const commands = scriptedCommands([readPreference(preference), readStatus(status)]);
        await assert.rejects(loadSharingSnapshot(commands.invoke));
        commands.done();
    }
});

test('enabling saves the preference, starts the native server and confirms its running state', async () => {
    const commands = scriptedCommands([setPreference(true), start(), readStatus('running')]);
    assert.deepEqual(await updateSharing(commands.invoke, true, stopped), { snapshot: running, error: null });
    commands.done();
});

test('disabling saves the preference, stops the native server and confirms its stopped state', async () => {
    const commands = scriptedCommands([setPreference(false), stop(), readStatus('stopped')]);
    assert.deepEqual(await updateSharing(commands.invoke, false, running), { snapshot: stopped, error: null });
    commands.done();
});

test('a failed preference write does not start or stop the server', async () => {
    for (const [target, previous] of [
        [true, stopped],
        [false, running],
    ]) {
        const commands = scriptedCommands([setPreference(target, true)]);
        assert.deepEqual(await updateSharing(commands.invoke, target, previous), { snapshot: previous, error: 'save' });
        commands.done();
    }
});

test('a failed start with an occupied port restores the disabled preference', async () => {
    const commands = scriptedCommands([
        setPreference(true),
        start(true),
        readStatus('port_in_use'),
        setPreference(false),
    ]);
    assert.deepEqual(await updateSharing(commands.invoke, true, stopped), {
        snapshot: { enabled: false, status: 'port_in_use' },
        error: 'start',
    });
    commands.done();
});

test('a failed or timed-out stop keeps the running server visible and restores the enabled preference', async () => {
    for (const stopFailed of [false, true]) {
        const commands = scriptedCommands([
            setPreference(false),
            stop(stopFailed),
            readStatus('running'),
            setPreference(true),
        ]);
        assert.deepEqual(await updateSharing(commands.invoke, false, running), { snapshot: running, error: 'stop' });
        commands.done();
    }
});

test('a status-read failure after start does not assume the server stopped or undo the saved choice', async () => {
    const commands = scriptedCommands([setPreference(true), start(), readStatus(undefined, true)]);
    assert.deepEqual(await updateSharing(commands.invoke, true, stopped), { snapshot: null, error: 'status' });
    commands.done();
});

test('a status-read failure after stop does not pretend that stop was confirmed', async () => {
    const commands = scriptedCommands([setPreference(false), stop(), readStatus(undefined, true)]);
    assert.deepEqual(await updateSharing(commands.invoke, false, running), { snapshot: null, error: 'status' });
    commands.done();
});

test('a failed rollback reloads preference and status and never displays a running server as disabled', async () => {
    const commands = scriptedCommands([
        setPreference(false),
        stop(),
        readStatus('running'),
        setPreference(true, true),
        readPreference(false),
        readStatus('running'),
    ]);
    assert.deepEqual(await updateSharing(commands.invoke, false, running), { snapshot: running, error: 'save' });
    commands.done();
});

test('an unconfirmed rollback leaves the UI indeterminate rather than showing a guessed state', async () => {
    const commands = scriptedCommands([
        setPreference(false),
        stop(),
        readStatus('running'),
        setPreference(true, true),
        readPreference(false, true),
        readStatus('running'),
    ]);
    assert.deepEqual(await updateSharing(commands.invoke, false, running), { snapshot: null, error: 'status' });
    commands.done();
});

test('a command error is reported even if a later status confirms the requested state', async () => {
    const commands = scriptedCommands([setPreference(false), stop(true), readStatus('stopped')]);
    assert.deepEqual(await updateSharing(commands.invoke, false, running), { snapshot: stopped, error: 'stop' });
    commands.done();
});
