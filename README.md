# Strauss RS

A laser-fast Rust take on [Strauss](https://github.com/BrianHenryIE/strauss): it prefixes the namespaces and global symbols of a Wordpress plugin or theme's Composer dependencies, so they can't clash with other plugins shipping the same libraries.

It's a single binary with no PHP required to run it.

## Getting started

### 1. Download `install.sh` into your project

From your plugin or theme's root folder (the one with `composer.json`):

```sh
mkdir -p bin
curl -fsSL -o bin/strauss-rs.sh https://raw.githubusercontent.com/craigrileyuk/strauss-rs/main/install.sh
```

The script picks the right release for your OS and CPU (macOS, Linux glibc/musl, Windows via Git Bash), checks its SHA-256 checksum, and keeps the binary in `bin/` next to itself.

### 2. Install and run it

```sh
sh bin/strauss-rs.sh prefix
```

The first run downloads the latest release, then runs `strauss-rs prefix`. Any arguments after the script name are passed to the binary. Running the script with no arguments only installs or updates.

The script adds what it downloads (the binary, the schema and an update-check stamp) to a `bin/.gitignore`, creating it if needed and leaving existing entries alone. Commit `bin/strauss-rs.sh` and `bin/.gitignore`.

### 3. Add a `strauss.json`

Next to `composer.json`:

```json
{
  "$schema": "./bin/strauss.schema.json",
  "target_directory": "vendor-prefixed",
  "namespace_prefix": "Acme\\Plugin\\Vendor",
  "classmap_prefix": "Acme_Plugin_",
  "packages": ["monolog/monolog"]
}
```

Everything is optional. Prefixes are inferred from `composer.json` when left out, and `packages` defaults to your `require` list. The schema, installed alongside the binary, gives editor completion for all options.

### 4. Preview, then prefix

```sh
sh bin/strauss-rs.sh diff             # show the changes as a diff
sh bin/strauss-rs.sh prefix --dry-run # work out what would be written, write nothing
sh bin/strauss-rs.sh prefix           # copy to target_directory and write the autoloader
```

Then load the generated autoloader from your plugin instead of (or after) Composer's.

### Migrating from Strauss PHP

strauss-rs does **not** read `extra.strauss` in `composer.json`. All of your options **must** be moved into a dedicated `strauss.json` next to `composer.json`. Anything left under `extra.strauss` is silently ignored, so without the move you get inferred defaults instead of your settings, and your files are prefixed differently from before.

Take the contents of the `extra.strauss` object and make them the whole of `strauss.json`, without the `extra` and `strauss` wrapper. The option names and values are the same. From:

```json
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
{
  "target_directory": "vendor-prefixed",
  "namespace_prefix": "Acme\\Plugin\\Vendor",
  "classmap_prefix": "Acme_Plugin_"
}
```

Then delete `extra.strauss` from `composer.json` so there is one source of truth. Things to know:

- Unknown keys are an error, so a typo or an unsupported option is reported rather than ignored.
- Mozart's legacy keys (`dep_directory`, `dep_namespace`, `exclude_packages` and so on) and the `extra.mozart` fallback are not supported. Use the current option names.
- If you leave out `classmap_prefix`, the one inferred from your package name ends in an underscore (`Acme_My_Plugin_`). Strauss PHP leaves the underscore off. Set it explicitly if you need your existing class names to stay the same.
- Your Composer scripts should call `strauss-rs` (see [Run it from Composer](#run-it-from-composer)) instead of `vendor/bin/strauss`.

## Updating your own code

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

## The autoloader

`prefix` writes `autoload.php` and `autoload-classmap.php` into the target directory. By default (`optimize_autoloader: true`) the autoloader is classmap-authoritative: every class is listed under its new name, so loading one never searches the disk.

Set `"optimize_autoloader": false` to also fall back to each package's PSR-4 directories, using the renamed namespaces, for classes that aren't in the classmap, such as files added after `prefix` ran. Packages that use `psr-0` are only loaded through the classmap, and you get a warning if there are any.

## Run it from Composer

```json
{
  "scripts": {
    "prefix-namespaces": ["sh bin/strauss-rs.sh prefix"],
    "post-install-cmd": ["@prefix-namespaces"],
    "post-update-cmd": ["@prefix-namespaces"]
  }
}
```

## Commands

| Command    | What it does                                                          |
| ---------- | --------------------------------------------------------------------- |
| `prefix`   | Copy packages to `target_directory`, rename symbols, write autoloader |
| `diff`     | Show the changes `prefix` would make                                  |
| `plan`     | Show which symbols would be renamed, and why others wouldn't          |
| `config`   | Print the resolved configuration as JSON                              |
| `packages` | List the packages to process                                          |
| `files`    | List the files of those packages                                      |
| `symbols`  | List the symbols each package declares                                |
| `analyse`  | Analyse one PHP file                                                  |

Useful options: `-C, --project-dir <DIR>` to run against another folder, `-q, --quiet`, `--parallel[=auto|false|N]`, `--updateCallSites[=true|false|PATHS]` on `prefix` and `diff`, and `--json` on `prefix`. Run `strauss-rs <command> --help` for the rest.

## Installer settings

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
