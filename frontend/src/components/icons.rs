//! Ícones SVG inline (traço, 24×24): sem dependência externa nem requisição de rede.

use leptos::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconKind {
    Dashboard,
    Expenses,
    Portfolio,
    Tax,
    Logo,
}

#[component]
pub fn Icon(kind: IconKind, #[prop(optional, into)] class: Option<&'static str>) -> impl IntoView {
    let paths = match kind {
        IconKind::Dashboard => view! {
            <rect x="3" y="3" width="7" height="9" rx="1.5" />
            <rect x="14" y="3" width="7" height="5" rx="1.5" />
            <rect x="14" y="12" width="7" height="9" rx="1.5" />
            <rect x="3" y="16" width="7" height="5" rx="1.5" />
        }
        .into_any(),
        IconKind::Expenses => view! {
            <path d="M6 3h12v18l-3-2-3 2-3-2-3 2z" />
            <path d="M9 8h6M9 12h6M9 16h3" />
        }
        .into_any(),
        IconKind::Portfolio => view! {
            <rect x="3" y="7" width="18" height="13" rx="2" />
            <path d="M8 7V5a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2M3 13h18" />
        }
        .into_any(),
        IconKind::Tax => view! {
            <path d="M3 9l9-5 9 5M4 9h16M6 9v9M10 9v9M14 9v9M18 9v9M3 20h18" />
        }
        .into_any(),
        IconKind::Logo => view! {
            <path d="M4 19V6l8 8 8-8v13" />
        }
        .into_any(),
    };

    view! {
        <svg
            class=class.unwrap_or("size-5")
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="1.8"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
        >
            {paths}
        </svg>
    }
}
