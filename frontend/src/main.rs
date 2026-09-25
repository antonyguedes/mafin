mod app;
mod components;
mod ipc;
mod pages;
mod util;

fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(app::App);
}
