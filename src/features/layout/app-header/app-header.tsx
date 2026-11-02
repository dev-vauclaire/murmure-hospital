import appIcon from '@/assets/app-icon.png';

const customLogos = import.meta.glob<string>('/src/assets/branding/logo.{png,svg,webp,jpg,jpeg}', {
    eager: true,
    query: '?url',
    import: 'default',
});
const customLogo = Object.values(customLogos)[0];

export const AppHeader = () => (
    <header className="flex w-full items-center justify-center gap-3 py-2">
        <img src={appIcon} alt="Murmure" width={64} height={64} className="h-16 w-16 shrink-0 object-contain" />
        {customLogo !== undefined && (
            // Match the transparent padding within the Murmure icon.
            <img
                src={customLogo}
                alt="Vauclaire"
                width={64}
                height={64}
                className="h-16 w-16 shrink-0 object-contain p-[7.5px]"
            />
        )}
    </header>
);
