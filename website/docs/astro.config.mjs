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
			disable404Route: true,
			defaultLocale: "root",
			locales: {
				root: { label: "English", lang: "en" },
				es: { label: "Español", lang: "es" },
			},
			head: [
				{
					tag: "script",
					attrs: {
						src: "https://analytics.ahrefs.com/analytics.js",
						"data-key": "u+wFBkUteccQg9KB5j71cA",
						async: true,
					},
				},
			],
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
					label: "Start Here",
					translations: { es: "Comienza aquí" },
					items: [
						{
							label: "Getting Started",
							translations: { es: "Primeros pasos" },
							slug: "guides/getting-started",
						},
					],
				},
				{
					label: "Core Concepts",
					translations: { es: "Conceptos clave" },
					items: [
						{
							label: "Instruction Files",
							translations: { es: "Archivos de instrucciones" },
							slug: "concepts/instruction-files",
						},
						{
							label: "Sync Types",
							translations: { es: "Tipos de sincronización" },
							slug: "concepts/sync-types",
						},
					],
				},
				{
					label: "Guides and Workflows",
					translations: { es: "Guías y flujos de trabajo" },
					items: [
						{
							label: "Configuration Recipes",
							translations: { es: "Recetas de configuración" },
							slug: "guides/recipes",
						},
						{
							label: "Windows Symlink Setup",
							translations: { es: "Configuración de symlinks en Windows" },
							slug: "guides/windows-symlink-setup",
						},
						{
							label: "Gitignore Team Workflows",
							translations: { es: "Flujos de trabajo de equipo con Gitignore" },
							slug: "guides/gitignore-team-workflows",
						},
						{
							label: "Git Hook Automation",
							translations: { es: "Automatización con Git hooks" },
							slug: "guides/git-hook-automation",
						},
						{
							label: "MCP Integration",
							translations: { es: "Integración de MCP" },
							slug: "guides/mcp",
						},
						{ label: "Skills", translations: { es: "Skills" }, slug: "guides/skills" },
						{
							label: "Inspect Configuration",
							translations: { es: "Inspeccionar configuración" },
							slug: "guides/inspect",
						},
						{
							label: "Troubleshooting",
							translations: { es: "Solución de problemas" },
							slug: "guides/troubleshooting",
						},
					],
				},
				{
					label: "CLI and Reference",
					translations: { es: "CLI y referencia" },
					items: [
						{
							label: "Command Line Reference",
							translations: { es: "Referencia de la línea de comandos" },
							slug: "reference/cli",
						},
					{
							label: "Configuration",
							translations: { es: "Configuración" },
							slug: "reference/configuration",
						},
					{
							label: "Inspect Output",
							translations: { es: "Salida de inspect" },
							slug: "reference/inspect-output",
						},
					{
							label: "Status Output",
							translations: { es: "Salida de status" },
							slug: "reference/status-output",
						},
					],
				},
				{
					label: "For Developers",
					translations: { es: "Para desarrolladores" },
					items: [
						{
							label: "Contributing",
							translations: { es: "Contribuir" },
							slug: "contributing/contributing",
						},
						{
							label: "Development",
							translations: { es: "Desarrollo" },
							slug: "contributing/development",
						},
						{
							label: "Workspaces",
							translations: { es: "Espacios de trabajo" },
							slug: "contributing/workspaces",
						},
						{
							label: "CLI and TUI Compatibility Contract",
							translations: { es: "Contrato de compatibilidad de CLI y TUI" },
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
