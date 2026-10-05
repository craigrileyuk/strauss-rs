<?php
namespace Acme\App;

use Acme\App\Contracts\Marker;

enum Status: string implements Marker
{
    case Active = 'active';
}
