<?php
namespace Acme\Core;

use Acme\Core\Sub\{Thing, Other as O};
use Acme\Core\{Client, Excluded\Item};
use Acme\Core\{function helper, const VERSION};

class Grouped extends Client
{
    public function f(Thing $a, O $b, Item $c)
    {
        helper();

        return VERSION;
    }
}
