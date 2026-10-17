import { defineConfig } from "vitepress";

// https://vitepress.dev/reference/site-config
export default defineConfig({
	base: "/strauss-rs/",
	title: "Strauss-RS",
	description: "A Rust port of Strauss, which prefixes the namespaces of a Composer project's dependencies",
	head: [["link", { rel: "icon", type: "image/svg+xml", href: "/logo.svg" }]],
	themeConfig: {
		logo: { light: "/logo.svg", dark: "/logo-light.svg" },
		siteTitle: false,

		nav: [
			{ text: "Guide", link: "/guide/getting-started" },
			{ text: "Reference", link: "/reference/configuration" },
			{ text: "Benchmarks", link: "/benchmarks" },
		],

		sidebar: [
			{
				text: "Guide",
				items: [
					{ text: "Why prefix?", link: "/guide/why" },
					{ text: "Getting started", link: "/guide/getting-started" },
					{ text: "Moving from Strauss PHP", link: "/guide/migrating" },
					{ text: "Updating your own code", link: "/guide/updating-your-code" },
					{ text: "The autoloader", link: "/guide/autoloader" },
					{ text: "Leaving things alone", link: "/guide/exclusions" },
					{ text: "Common problems", link: "/guide/common-problems" },
				],
			},
			{
				text: "Reference",
				items: [
					{ text: "Configuration", link: "/reference/configuration" },
					{ text: "Command line", link: "/reference/cli" },
					{ text: "Installer script", link: "/reference/installer" },
				],
			},
			{
				text: "More",
				items: [{ text: "Benchmarks", link: "/benchmarks" }],
			},
		],

		socialLinks: [{ icon: "github", link: "https://github.com/evo-mark/strauss-rs" }],

		search: { provider: "local" },

		footer: {
			message: "Released under the MIT licence. Strauss-RS is based on Strauss by Brian Henry.",
			copyright: "Copyright evoMark",
		},
	},
});
