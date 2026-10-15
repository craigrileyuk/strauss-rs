<?php
namespace Acme\Core;

class Client
{
    public function make(): Sub\Thing
    {
        helper();

        return new Sub\Thing();
    }

    public function item(): Excluded\Item
    {
        return new Excluded\Item();
    }
}
