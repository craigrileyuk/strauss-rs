<?php
namespace Acme\Core;

use Acme\Core\Sub\Thing;
use Acme\Core\Sub as S;
use function Acme\Core\helper;
use const Acme\Core\VERSION;
use Countable;

class Util extends Client implements Countable
{
    const KIND = Thing::class;

    public function f(Thing $a, S\Thing $b, \Acme\Core\Sub\Thing $c, \Exception $d): ?Client
    {
        helper();
        \Acme\Core\helper();
        $v = VERSION . \Acme\Core\VERSION;
        $e = new \Acme\Core\Client();

        try {
            return strlen("x") > 0 ? null : $this;
        } catch (\InvalidArgumentException $x) {
            return $e;
        }
    }

    public function count(): int
    {
        return 0;
    }
}
