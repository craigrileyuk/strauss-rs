use super::*;

#[test]
fn recognises_platform_requirements() {
    for name in [
        "php",
        "PHP",
        "php-64bit",
        "php-ipv6",
        "php-zts",
        "php-debug",
        "hhvm",
        "ext-json",
        "EXT-JSON",
        "ext-pdo_mysql",
        "ext-zend.opcache",
        "lib-curl",
        "lib-icu",
        "composer",
        "composer-plugin-api",
        "composer-runtime-api",
    ] {
        assert!(is_platform_package(name), "{name}");
    }
}

#[test]
fn real_packages_are_not_platform_requirements() {
    for name in [
        // PHP Strauss treats anything starting with `ext` as a platform
        // package, which this one would be mistaken for.
        "extra/thing",
        "ext/thing",
        "php-http/client-implementation",
        "symfony/polyfill-php80",
        "phpunit/phpunit",
        "composer/installers",
        "composer/composer",
        "ext-",
        "ext-_x",
        "lib-",
        "acme/lib",
    ] {
        assert!(!is_platform_package(name), "{name}");
    }
}
