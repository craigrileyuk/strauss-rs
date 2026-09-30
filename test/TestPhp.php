<?php

namespace Foo;

function test(): void
{
    foo();
    Bar\baz();
    \Global\thing();
    $callable();
}
