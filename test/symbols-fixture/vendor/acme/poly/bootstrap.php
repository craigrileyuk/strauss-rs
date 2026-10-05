<?php

if (!interface_exists('Stringable', false)) {
    interface Stringable {}
}

if (!function_exists('str_contains')) {
    function str_contains($haystack, $needle) {}
}

if (!function_exists('STR_STARTS_WITH')) {
    function STR_STARTS_WITH($haystack, $needle) {}
}

if (!class_exists('ArrayObject', false)) {
    class ArrayObject {}
}

function acme_poly_helper() {}

const POLY_FLAG = 1;

define('POLY_DEFINED', 2);
