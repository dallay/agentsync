// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import Icons from 'unplugin-icons/vite'

// https://astro.build/config
export default defineConfig({
    site: 'https://dallay.github.io',
    base: '/agentsync',
    integrations: [
        starlight({
            title: 'AgentSync',
            // Use our local Hero component to override the theme's Hero
            components: {
                Hero: './src/components/Hero.astro',
                Footer: './src/components/Footer.astro',
            },
            // Load theme fonts via fontsource packages (install them with pnpm add)
            customCss: [
                '@fontsource/geist-mono',
                '@fontsource/geist-sans',
                './src/styles/custom.css',
            ],
            social: [{ icon: 'github', label: 'GitHub', href: 'https://github.com/dallay/agentsync' }],
            sidebar: [
                {
                    label: 'Guides',
                    items: [
                        { label: 'Getting Started', slug: 'guides/getting-started' },
                        { label: 'MCP Integration', slug: 'guides/mcp' },
                        { label: 'Contributing', slug: 'guides/contributing' },
                    ],
                },
                {
                    label: 'Reference',
                    autogenerate: { directory: 'reference' },
                },
            ],
        }),
    ],
    vite: {
        plugins: [Icons({ compiler: 'astro' })],
    }
});
