# Getting started

This takes about five minutes. You need a Wordpress plugin or theme that uses Composer, with a `composer.json` and a `vendor/` folder.

## 1. Install the script

From your plugin or theme's root folder (the one with `composer.json`):

```sh
mkdir -p bin
curl -fsSL -o bin/strauss-rs.sh https://raw.githubusercontent.com/evo-mark/strauss-rs/main/install.sh
sh bin/strauss-rs.sh
```

The script downloads the right release for your operating system and CPU, checks its checksum, and keeps the program in `bin/` next to itself. It also keeps a `bin/.gitignore` up to date so the download is not committed. Commit `bin/strauss-rs.sh` and `bin/.gitignore`.

## 2. Create a `strauss.json`

```sh
sh bin/strauss-rs.sh init
```

This reads your `composer.json` and writes a `strauss.json` next to it. It works out the prefixes for you and asks whether to add the Composer scripts from step 3. Add `--dry-run` to see what it would write without writing anything.

The file looks like this:

```json
{
	"$schema": "./bin/strauss.schema.json",
	"namespace_prefix": "Acme\\Plugin\\Vendor",
	"classmap_prefix": "Acme_Plugin_",
	"include_autoloader": true
}
```

- `namespace_prefix` is put in front of every dependency namespace.
- `classmap_prefix` is put in front of classes that have no namespace.
- `include_autoloader` makes Composer's `vendor/autoload.php` load the prefixed packages as well, so your plugin does not need an extra `require`.

Everything in this file is optional. Every option is listed on the [configuration page](/reference/configuration).

## 3. Run it from Composer

Add this to your `composer.json` (`init` can do it for you):

```json
{
	"scripts": {
		"prefix-namespaces": ["sh bin/strauss-rs.sh prefix"],
		"post-install-cmd": ["@prefix-namespaces"],
		"post-update-cmd": ["@prefix-namespaces"]
	}
}
```

Now `composer install` and `composer update` prefix your dependencies into a `vendor-prefixed/` folder as their last step.

## 4. Run it

```sh
composer install
```

You should see a summary of what was copied and renamed. Your dependencies are now in `vendor-prefixed/`, with names like `Acme\Plugin\Vendor\GuzzleHttp\Client`.

## 5. Use the prefixed names in your code

Your own files still say `use GuzzleHttp\Client;`. Either change them by hand to the prefixed name, or let Strauss-RS do it:

```json
{
	"update_call_sites": true
}
```

This rewrites your source files in place, so commit your work first and look at the changes with `sh bin/strauss-rs.sh diff` before you run it. The [page on updating your own code](./updating-your-code) has the details.

## Checking what it did

```sh
sh bin/strauss-rs.sh diff
```

shows every change as a diff without writing anything. To find out why one name was or was not renamed:

```sh
sh bin/strauss-rs.sh explain 'GuzzleHttp\Client'
```

```
class GuzzleHttp\Client
  declared by  guzzlehttp/guzzle
  in           guzzlehttp/guzzle/src/Client.php
  autoloaded   yes
  renamed to   Acme\Plugin\Vendor\GuzzleHttp\Client
  because      it is autoloaded
```

If something goes wrong after prefixing, see [common problems](./common-problems).
