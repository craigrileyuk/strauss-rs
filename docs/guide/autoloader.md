# The autoloader

`prefix` writes two files into the target directory: `autoload.php` and `autoload-classmap.php`. Your plugin has to load `autoload.php` before it uses any prefixed class.

## Loading it

The simplest way is `include_autoloader`:

```json
{
	"include_autoloader": true
}
```

After prefixing, this adds a line to `vendor/autoload.php` that loads the prefixed packages. Composer rewrites `vendor/autoload.php` whenever it installs or updates, so run `prefix` after those. The Composer scripts in [getting started](./getting-started#3-run-it-from-composer) do that. You can also run it once with `sh bin/strauss-rs.sh include-autoloader`. It is safe to run again.

If you would rather load it yourself, require the file from your plugin:

```php
require_once __DIR__ . '/vendor-prefixed/autoload.php';
```

## Code that runs very early

Some Wordpress code runs before any plugin has loaded: an `object-cache.php` drop-in, an `advanced-cache.php` drop-in, or a firewall that boots from `wp-config.php`. If that code uses a prefixed class, something has to load the prefixed autoloader first. With `include_autoloader` on, anything that loads Composer's `vendor/autoload.php` gets it. If your early code does not load Composer, it must require `vendor-prefixed/autoload.php` itself.

## Classmap or fallback

By default (`optimize_autoloader: true`) the autoloader is classmap-authoritative. Every class is listed under its new name, so loading a class never searches the disk.

Set `"optimize_autoloader": false` to also fall back to each package's PSR-4 folders, using the renamed namespaces, for classes that are not in the classmap, such as files added after `prefix` ran. Packages that use `psr-0` are only loaded through the classmap, and you get a warning if there are any.

## Files that always load

Packages can list `files` in their autoload section, usually for helper functions. The generated `autoload.php` requires those files for you.

## Turning the autoloader off

`classmap_output` controls whether the two files are written at all. If you leave it out, it is `false` when the target directory is already part of your project's own `autoload` (Composer is loading it), and `true` otherwise.

## Deleting the originals

Once the packages have been copied and prefixed, the originals in `vendor/` are extra weight in a release. Set `delete_vendor_packages` to remove them:

```json
{
	"delete_vendor_packages": true
}
```

Composer's own autoloader still lists those packages and would stop on their missing `files`, so after deleting, `prefix`:

1. removes the deleted packages from `vendor/composer/installed.json`, and
2. runs `composer dump-autoload --no-scripts` to rebuild Composer's autoloader from what is left.

If `composer` is not on your `PATH`, set `STRAUSS_RS_COMPOSER` to the command to run. If it cannot be run, `prefix` warns and tells you what to do by hand. Nothing is deleted in a `--dry-run`.

Because the packages are gone from `installed.json`, the next `prefix` needs them back, so run `composer install` first. Hooking `prefix` onto `post-install-cmd` and `post-update-cmd` does this for you.
