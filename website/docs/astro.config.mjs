// @ts-check

import starlight from "@astrojs/starlight";
import { defineConfig } from "astro/config";
import Icons from "unplugin-icons/vite";

// https://astro.build/config
export default defineConfig({
	site: "https://dallay.github.io",
	base: "/agentsync",
	integrations: [
		starlight({
			title: "AgentSync",
			// Use our local Hero component to override the theme's Hero
			components: {
				Hero: "./src/components/Hero.astro",
				Footer: "./src/components/Footer.astro",
			},
			// Load theme fonts via fontsource packages (install them with pnpm add)
			customCss: [
				"@fontsource/geist-mono",
				"@fontsource/geist-sans",
				"./src/styles/custom.css",
			],
			social: [
				{
					icon: "github",
					label: "GitHub",
					href: "https://github.com/dallay/agentsync",
				},
			],
			sidebar: [
				{
					label: "Core Concepts",
					items: [{ label: "Sync Types", slug: "concepts/sync-types" }],
				},
				{
					label: "Guides",
					items: [
						{ label: "Getting Started", slug: "guides/getting-started" },
						{
							label: "Windows Symlink Setup",
							slug: "guides/windows-symlink-setup",
						},
						{
							label: "Gitignore Team Workflows",
							slug: "guides/gitignore-team-workflows",
						},
						{
							label: "Git Hook Automation",
							slug: "guides/git-hook-automation",
						},
						{ label: "MCP Integration", slug: "guides/mcp" },
						{ label: "Skills", slug: "guides/skills" },
					],
				},
				{
					label: "Reference",
					items: [
						{ label: "CLI", slug: "reference/cli" },
						{ label: "Configuration", slug: "reference/configuration" },
						{ label: "Status Output", slug: "reference/status-output" },
					],
				},
				{
					label: "For Developers",
					items: [
						{ label: "Contributing", slug: "contributing/contributing" },
						{ label: "Development", slug: "contributing/development" },
						{ label: "Workspaces", slug: "contributing/workspaces" },
						{
							label: "CLI and TUI Compatibility Contract",
							slug: "contributing/cli-tui-contract",
						},
					],
				},
			],
		}),
	],
	vite: {
		plugins: [Icons({ compiler: "astro" })],
	},
});
