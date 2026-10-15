<?php
namespace Acme\Consumer;

use GlobalClass;
use function g_helper;
use Acme\Core\Client;

class UsesGlobals extends GlobalClass implements \GlobalContract
{
    public function run(Client $client)
    {
        g_helper();
        \g_helper();
        g_other();
        $x = new GlobalClass();
        $y = \GlobalClass::class;
        $z = G_CONST;
        $w = \G_CONST;

        defined("G_DEFINED");
        function_exists("g_helper");
        function_exists("\\g_other");

        return namespace\Missing::class;
    }
}
