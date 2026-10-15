<?php

if (!class_exists('WC_Order', false)) {
    class WC_Order {}
}

if (!function_exists('WC_GET_ORDER')) {
    function WC_GET_ORDER($id) {}
}

if (!defined('WC_ABSPATH')) {
    define('WC_ABSPATH', '/');
}
