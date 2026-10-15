<?php
namespace Acme\Keep;

use Acme\Core\Client;

class Kept extends Client
{
    public function f(): \Acme\Core\Sub\Thing
    {
        return new Sub\Thing();
    }
}
