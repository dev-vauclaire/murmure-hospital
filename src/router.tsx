import { createRouter, createRoute, createRootRoute, Navigate } from '@tanstack/react-router';
import { Home } from './features/home/home';
import { Layout } from './features/layout/layout';
import { About } from './features/about/about';
import { CustomDictionary } from './features/personalize/custom-dictionary/custom-dictionary';
import { FormattingRules } from './features/personalize/formatting-rules/formatting-rules';
import { System } from './features/settings/system/system';

const rootRoute = createRootRoute({
    component: () => <Layout />,
});

const indexRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/',
    component: Home,
});

const settingsShortcutsRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/settings/shortcuts',
    component: () => <Navigate to="/settings/system" replace />,
});

const personalizeCustomDictionaryRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/personalize/custom-dictionary',
    component: CustomDictionary,
});

const personalizeFormattingRulesRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/personalize/formatting-rules',
    component: FormattingRules,
});

const extensionsLLMConnectRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/extensions/llm-connect',
    component: () => <Navigate to="/" replace />,
});

const extensionsCommandModeRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/extensions/command-mode',
    component: () => <Navigate to="/" replace />,
});

const personalizeLLMConnectRedirectRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/personalize/llm-connect',
    component: () => <Navigate to="/" replace />,
});

const settingsSystemRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/settings/system',
    component: System,
});

const settingsIndexRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/settings',
    component: () => <Navigate to="/settings/system" replace />,
});

const personalizeIndexRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/personalize',
    component: () => <Navigate to="/personalize/custom-dictionary" />,
});

const extensionsVoiceModeRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/extensions/voice-mode',
    component: () => <Navigate to="/" replace />,
});

const extensionsSmartMicRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/extensions/smart-mic',
    component: () => <Navigate to="/" replace />,
});

const extensionsLocalApiRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/extensions/local-api',
    component: () => <Navigate to="/" replace />,
});

const extensionsIndexRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/extensions',
    component: () => <Navigate to="/" replace />,
});

const personalizeVoiceModeRedirectRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/personalize/voice-mode',
    component: () => <Navigate to="/" replace />,
});

const settingsImportExportRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/settings/import-export',
    component: () => <Navigate to="/settings/system" replace />,
});

const aboutRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/about',
    component: About,
});

const routeTree = rootRoute.addChildren([
    indexRoute,
    settingsIndexRoute,
    settingsShortcutsRoute,
    settingsSystemRoute,
    settingsImportExportRoute,
    personalizeIndexRoute,
    personalizeCustomDictionaryRoute,
    personalizeFormattingRulesRoute,
    personalizeLLMConnectRedirectRoute,
    personalizeVoiceModeRedirectRoute,
    extensionsIndexRoute,
    extensionsLLMConnectRoute,
    extensionsCommandModeRoute,
    extensionsVoiceModeRoute,
    extensionsSmartMicRoute,
    extensionsLocalApiRoute,
    aboutRoute,
]);

export const router = createRouter({ routeTree });
