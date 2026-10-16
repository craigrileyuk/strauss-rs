<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="strauss-rs-logo-light.svg">
    <img src="strauss-rs-logo.svg" alt="Strauss RS" width="300">
  </picture>
</p>

A super-fast Rust take on [Strauss](https://github.com/BrianHenryIE/strauss). Built for Wordpress plugins/themes where you cannot control the project dependencies centrally.

Prefix the namespaces and global symbols inside a Composer project so they cannot clash with other parts of your project using the same libraries.

Strauss-RS is a single binary with no PHP or other dependencies required to run it.

## Contents

- [Getting started](#getting-started)
    - [1. Install the script](#1-install-the-script)
    - [2. Create a `strauss.json`](#2-create-a-straussjson)
    - [3. Run it from Composer](#3-run-it-from-composer)
- [Why Use Strauss?](#why-use-strauss)
- [Advanced usage](#advanced-usage)
    - [Previewing changes](#previewing-changes)
    - [Explaining a symbol](#explaining-a-symbol)
    - [Migrating from Strauss PHP](#migrating-from-strauss-php)
    - [Updating your own code](#updating-your-own-code)
    - [The autoloader](#the-autoloader)
    - [Deleting the originals](#deleting-the-originals)
    - [Presets](#presets)
    - [Commands](#commands)
    - [Installer settings](#installer-settings)

## Getting started

### 1. Install the script

From your plugin or theme's root folder (the one with `composer.json`):

```sh
mkdir -p bin
curl -fsSL -o bin/strauss-rs.sh https://raw.githubusercontent.com/evo-mark/strauss-rs/main/install.sh
sh bin/strauss-rs.sh
```

The script downloads the right release for your OS and CPU, checks its checksum, and keeps the binary in `bin/` next to itself. It also keeps `bin/.gitignore` up to date so the download isn't committed. Commit `bin/strauss-rs.sh` and `bin/.gitignore`.

### 2. Create a `strauss.json`

Next to your folder's `composer.json` file, you should create a `strauss.json` file:

```json
{
	"$schema": "./bin/strauss.schema.json",
	"namespace_prefix": "Acme\\Plugin\\Vendor",
	"classmap_prefix": "Acme_Plugin_",
	"include_autoloader": true
}
```

Alternatively, use `sh bin/strauss-rs.sh init` to automatically create one.

Everything in this file is optional. Prefixes are inferred from `composer.json` when left out, and every package in your composer `require` block is processed. The schema gives your editor completion for all options. `include_autoloader` makes Composer's `vendor/autoload.php` load the prefixed packages, so your plugin needs no extra `require`.

Coming from Strauss PHP? Your options must be moved here from `composer.json`; see [Migrating from Strauss PHP](#migrating-from-strauss-php).

### 3. Run it from Composer

Add to your `composer.json`:

```json
{
	"scripts": {
		"prefix-namespaces": ["sh bin/strauss-rs.sh prefix"],
		"post-install-cmd": ["@prefix-namespaces"],
		"post-update-cmd": ["@prefix-namespaces"]
	}
}
```

Now `composer install` and `composer update` prefix your dependencies into `vendor-prefixed/` as their last step.

## Why Use Strauss?

Say that two plugins bundle different versions of the same library. Wordpress loads both into one PHP process, so whichever autoloader registers first wins and the other plugin gets the wrong version.

```
wp-content/plugins/
├── plugin-a/
│   ├── plugin-a.php
│   └── vendor/
│       └── guzzlehttp/guzzle/     # v6.5 - declares GuzzleHttp\Client
└── plugin-b/
    ├── plugin-b.php
    └── vendor/
        └── guzzlehttp/guzzle/     # v7.8 - also declares GuzzleHttp\Client
```

After running Strauss-RS in each plugin, every copy lives under its own namespace:

```
wp-content/plugins/
├── plugin-a/
│   ├── plugin-a.php
│   └── vendor-prefixed/
│       └── guzzlehttp/guzzle/     # Acme\PluginA\Vendor\GuzzleHttp\Client (v6.5)
└── plugin-b/
    ├── plugin-b.php
    └── vendor-prefixed/
        └── guzzlehttp/guzzle/     # Acme\PluginB\Vendor\GuzzleHttp\Client (v7.8)
```

No more clashes

## Advanced usage

### Previewing changes

```sh
sh bin/strauss-rs.sh diff             # show the changes as a diff
sh bin/strauss-rs.sh prefix --dry-run # work out what would be written, write nothing
sh bin/strauss-rs.sh prefix           # copy to target_directory and write the autoloader
```

Any arguments after the script name are passed to the binary. Running the script with no arguments only installs or updates. `STRAUSS_RS_VERSION` pins a release; see [Installer settings](#installer-settings).

### Explaining a symbol

```sh
sh bin/strauss-rs.sh explain 'GuzzleHttp\Client'
```

Says whether the symbol is renamed, what to, and which setting decided it, with what to change to get the other result:

```
class GuzzleHttp\Client
  declared by  guzzlehttp/guzzle
  in           guzzlehttp/guzzle/src/Client.php
  autoloaded   yes
  renamed to   Acme\Plugin\Vendor\GuzzleHttp\Client
  because      it is autoloaded
```

Names are matched the way PHP does (case-insensitively, except for constants) and a leading backslash is ignored. Use `--kind` when one name is more than one thing, such as a class and a function, and `--json` for a machine-readable answer. Symbols that PHP or an [`exclude_presets`](#presets) preset provides are explained too, even if no package declares them. If nothing matches, the command fails and lists similar names.

### Starting a new project

`init` writes a `strauss.json` from your `composer.json`:

```sh
sh bin/strauss-rs.sh init
```

It works out `namespace_prefix` and `classmap_prefix` and turns on `include_autoloader`. Dependencies go under `<your namespace>\Vendor`, so they can't clash with your own classes. Override what it works out with `--namespace-prefix`, `--classmap-prefix` and `--target-directory`, and look first with `--dry-run`. It won't replace an existing `strauss.json` without `--force`.

It can also add a `prefix-namespaces` script to `composer.json`, run from `post-install-cmd` and `post-update-cmd` (see [step 3](#3-run-it-from-composer)). It asks when run in a terminal; `--composer-scripts` and `--no-composer-scripts` answer for you. Your `composer.json` keeps its key order and indentation.

### Migrating from Strauss PHP

`migrate` does what this section describes for you:

```sh
sh bin/strauss-rs.sh migrate --dry-run   # show the strauss.json it would write
sh bin/strauss-rs.sh migrate
```

It moves `extra.strauss` into `strauss.json` and removes it from `composer.json` (other `extra` entries stay). Nothing is written if an option isn't one strauss-rs supports. Where PHP Strauss would have inferred a `classmap_prefix` differently, it sets that value explicitly so your class names don't change. It warns about scripts that still run `vendor/bin/strauss`, but doesn't edit them. `--force` replaces an existing `strauss.json`.

To do it by hand instead: Strauss-RS does **not** read `extra.strauss` in `composer.json`. All of your options **must** be moved into a dedicated `strauss.json` next to `composer.json`. Anything left under `extra.strauss` is silently ignored, so without the move you get inferred defaults instead of your settings, and your files are prefixed differently from before.

Take the contents of the `extra.strauss` object and make them the whole of `strauss.json`, without the `extra` and `strauss` wrapper. The option names and values are the same. From:

```json
// composer.json
{
	"name": "acme/plugin",
	"extra": {
		"strauss": {
			"target_directory": "vendor-prefixed",
			"namespace_prefix": "Acme\\Plugin\\Vendor",
			"classmap_prefix": "Acme_Plugin_"
		}
	}
}
```

to a `strauss.json` containing:

```json
// strauss.json
{
	"target_directory": "vendor-prefixed",
	"namespace_prefix": "Acme\\Plugin\\Vendor",
	"classmap_prefix": "Acme_Plugin_"
}
```

Then delete `extra.strauss` from `composer.json` so there is one source of truth. Things to know:

- Unknown keys are an error, so a typo or an unsupported option is reported rather than ignored.
- Mozart's (the project Strauss was originally forked from) legacy keys (`dep_directory`, `dep_namespace`, `exclude_packages` and so on) and the `extra.mozart` fallback are not supported. Use the current option names.
- If you leave out `classmap_prefix`, the one inferred from your package name ends in an underscore (`Acme_My_Plugin_`). Strauss PHP leaves the underscore off. Set it explicitly if you need your existing class names to stay the same.
- Your Composer scripts should call `strauss-rs` (see [Run it from Composer](#3-run-it-from-composer)) instead of `vendor/bin/strauss`.

### Updating your own code

Your plugin's own files refer to the packages too. Set `update_call_sites` and `prefix` rewrites them in place, so `use Acme\Core\Client;` becomes `use Acme\Vendor\Acme\Core\Client;`:

```json
{
	"update_call_sites": true
}
```

- `true` updates the `.php` files under the `autoload` paths in your `composer.json` (`psr-4`, `psr-0`, `classmap` and `files`). `autoload-dev` is not included.
- A list, such as `["src", "includes/loader.php"]`, updates just those files and folders, relative to the project.
- Your own namespaces are left alone. The vendor and target directories are never touched, and `exclude_files_from_update` patterns still apply.
- Override it for one run with `--updateCallSites`, `--updateCallSites=false` or `--updateCallSites=src,includes` (on `prefix` and `diff`).

This changes your source files, so commit first and look at `diff --updateCallSites` before running `prefix`. `prefix --dry-run` reports how many files would change without writing them.

### The autoloader

`prefix` writes `autoload.php` and `autoload-classmap.php` into the target directory. By default (`optimize_autoloader: true`) the autoloader is classmap-authoritative: every class is listed under its new name, so loading one never searches the disk.

Set `"optimize_autoloader": false` to also fall back to each package's PSR-4 directories, using the renamed namespaces, for classes that aren't in the classmap, such as files added after `prefix` ran. Packages that use `psr-0` are only loaded through the classmap, and you get a warning if there are any.

### Deleting the originals

`delete_vendor_packages` removes the original packages from `vendor/` once they've been copied and prefixed, so you don't ship two copies. Composer's own autoloader still lists those packages and would fatally error on their missing `files`, so after deleting, `prefix`:

1. removes the deleted packages from `vendor/composer/installed.json`, and
2. runs `composer dump-autoload --no-scripts` to regenerate Composer's autoloader from what is left.

If `composer` isn't on your `PATH`, set `STRAUSS_RS_COMPOSER` to the command to run. If it can't be run, `prefix` warns and tells you what to do by hand. Nothing is deleted in a `--dry-run`.

Because the packages are gone from `installed.json`, the next `prefix` needs them back: run `composer install` first. Hooking `prefix` onto Composer's `post-install-cmd` and `post-update-cmd` (see [step 3](#3-run-it-from-composer)) does this for you. `--no-scripts` stops the regeneration from triggering those hooks again.

### Presets

Some libraries declare symbols that the platform they run on already has, usually in a shim guarded by `function_exists()` or `class_exists()`. If that shim were renamed, the platform would never see it and the shim would stop doing its job. `exclude_presets` lists sets of symbols that are never renamed:

```json
{
  "exclude_presets": ["wordpress", "woocommerce", "wp-cli"]
}
```

| Preset        | Covers                                                                                                   | On by default |
| ------------- | -------------------------------------------------------------------------------------------------------- | ------------- |
| `wordpress`   | Wordpress core's global classes (`WP_Error`, `wpdb`), functions (`add_action`) and constants (`ABSPATH`) | Yes           |
| `woocommerce` | WooCommerce's global classes (`WC_Order`), functions (`wc_get_order`) and constants (`WC_ABSPATH`)        | No            |
| `wp-cli`      | WP-CLI's global classes (`WP_CLI`, `WP_CLI_Command`, `Plugin_Command`) and constants (`WP_CLI`)           | No            |

- Setting `exclude_presets` replaces the default, so list `wordpress` too if you want it alongside the others. `"exclude_presets": []` renames those symbols like any others.
- Namespaced symbols are not included, so a bundled copy of something like `WpOrg\Requests` or `Automattic\WooCommerce\Client` is still prefixed.
- PHP's own classes and functions are always left alone, whatever you set here.
- `strauss-rs symbols` lists what was left alone under `built_ins`, with `provided_by` set to `php` or the preset's name, and `strauss-rs explain` says why a particular symbol was left alone.

The class and function lists are generated from [php-stubs/wordpress-stubs](https://github.com/php-stubs/wordpress-stubs) and [php-stubs/woocommerce-stubs](https://github.com/php-stubs/woocommerce-stubs) and [php-stubs/wp-cli-stubs](https://github.com/php-stubs/wp-cli-stubs) with `scripts/generate-presets.sh`. The stubs mostly don't declare constants, so those come from `scripts/wordpress-constants.txt`, `scripts/woocommerce-constants.txt` and `scripts/wp-cli-constants.txt`.

### Commands

| Command    | What it does                                                          |
| ---------- | --------------------------------------------------------------------- |
| `prefix`   | Copy packages to `target_directory`, rename symbols, write autoloader |
| `diff`     | Show the changes `prefix` would make                                  |
| `plan`     | Show which symbols would be renamed, and why others wouldn't          |
| `explain`  | Explain what happens to one symbol, and why                           |
| `init`     | Write a starting `strauss.json`                                       |
| `migrate`  | Move `extra.strauss` from `composer.json` into `strauss.json`         |
| `config`   | Print the resolved configuration as JSON                              |
| `packages` | List the packages to process                                          |
| `files`    | List the files of those packages                                      |
| `symbols`  | List the symbols each package declares                                |
| `analyse`  | Analyse one PHP file                                                  |

Useful options: `-C, --project-dir <DIR>` to run against another folder, `-q, --quiet`, `--parallel[=auto|false|N]`, `--updateCallSites[=true|false|PATHS]` on `prefix` and `diff`, and `--json` on `prefix`. Run `strauss-rs <command> --help` for the rest.

### Installer settings

Set these as environment variables when running the script:

| Variable               | Purpose                                                     |
| ---------------------- | ----------------------------------------------------------- |
| `STRAUSS_RS_VERSION`   | Pin a release tag, e.g. `v0.1.0`, instead of the latest     |
| `STRAUSS_RS_DIR`       | Where the binary is kept (default: the script's own folder) |
| `STRAUSS_RS_OFFLINE`   | `1` to never look for an update                             |
| `STRAUSS_RS_GITIGNORE` | `0` to leave `.gitignore` alone                             |
| `STRAUSS_RS_REPO`      | GitHub repository to download from                          |
| `STRAUSS_RS_BASE_URL`  | Release download location                                   |

Without `STRAUSS_RS_VERSION`, the latest release is checked for once a day. If GitHub can't be reached, the installed binary is used.
