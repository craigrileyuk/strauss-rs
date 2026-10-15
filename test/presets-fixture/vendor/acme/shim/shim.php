<?php

if (!class_exists('WP_Error', false)) {
    class WP_Error {}
}

if (!function_exists('WP_JSON_ENCODE')) {
    function WP_JSON_ENCODE($value) {}
}

if (!defined('ABSPATH')) {
    define('ABSPATH', '/');
}

const WP_DEBUG = false;

// Not Wordpress': a different case, a namespace and an unknown name.
const wp_debug_lower = 1;

function acme_shim_helper() {}

class Acme_Shim_Thing {}
