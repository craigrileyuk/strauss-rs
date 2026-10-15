<?php
namespace Acme\Lib;

require __DIR__ . '/src/Helper.php';
require __DIR__ . '/src/Limits.php';

function boot()
{
    return [new Helper(), new \Acme\Lib\Helper(), LIMIT];
}
