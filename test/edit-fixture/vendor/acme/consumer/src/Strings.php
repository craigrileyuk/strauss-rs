<?php
namespace Acme\Consumer;

use Acme\Core\Client;

class Strings
{
    /**
     * @param \Acme\Core\Client $client
     * @param Client $same
     * @return \GlobalClass|null
     */
    public function run($client, $same)
    {
        $map = ['Acme\\Core\\' => 'src/'];
        $class = 'Acme\Core\Client';
        $kept = 'Acme\Core\Excluded\Item';
        add_action('init', 'g_helper');
        $row = ['g_helper' => 1];

        return "Unknown Acme\Core\Sub\Thing: {$class}";
    }
}
