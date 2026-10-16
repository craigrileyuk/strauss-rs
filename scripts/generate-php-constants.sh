#!/bin/sh
# Regenerates src/symbols/presets/php-constants.txt: the global constants PHP and
# its extensions define, for the opt-in `php-constants` preset.
#
#   scripts/generate-php-constants.sh
#
# Needs a PHP on the PATH. It lists what that PHP defines, so run it with the PHP
# you want to cover and as many extensions loaded as you can: a constant from an
# extension or a PHP version that isn't there is missing from the list. Add those
# by hand to scripts/php-constants-extra.txt.
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
out="$root/src/symbols/presets/php-constants.txt"

version=$(php -r 'echo PHP_VERSION;')

{
    echo "# Constants PHP and its extensions define."
    echo "#"
    echo "# Symbols with these names are never renamed when the \`php-constants\` preset is on"
    echo "# (\`exclude_presets\`): a polyfill declares them for a PHP without the extension,"
    echo "# and a renamed copy is not what PHP's own code, or an extension, defines."
    echo "#"
    echo "# Generated from PHP $version by scripts/generate-php-constants.sh, plus"
    echo "# scripts/php-constants-extra.txt. Don't edit by hand."
    echo
    {
        php -r '
            $all = get_defined_constants(true);
            unset($all["user"], $all["lerd_devtools"]);
            foreach ($all as $constants) {
                foreach (array_keys($constants) as $name) {
                    if (!str_contains($name, "\\")) {
                        echo $name, "\n";
                    }
                }
            }
        '
        grep -v '^#' "$root/scripts/php-constants-extra.txt" | grep -v '^$'
    } | sed 's/^/k /' | LC_ALL=C sort -u
} > "$out"

echo "wrote src/symbols/presets/php-constants.txt from PHP $version"
