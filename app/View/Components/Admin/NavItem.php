<?php

namespace App\View\Components\Admin;

use Closure;
use Illuminate\Contracts\View\View;
use Illuminate\View\Component;

class NavItem extends Component
{
    public function __construct(
        public string $href,
        public string $label,
        public string $icon = 'dot',
        public bool $active = false,
    ) {}

    public function render(): View|Closure|string
    {
        return view('components.admin.nav-item');
    }
}
