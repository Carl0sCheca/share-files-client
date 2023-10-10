mod app;
mod settings;

use app::*;
use leptos::*;
use settings::*;
use stylers::style;

use wasm_bindgen::prelude::*;
use web_sys::{Event, HtmlInputElement};

fn main() {
    let (settings, set_settings) = create_signal(false);

    let styler_class = style! { "Main",
        div.settings-btn {
            cursor: pointer;
            border-radius: 5px;
            padding: 5px;
            display: inline-block;
        }
        div.settings-btn:hover {
            background-color: rgba(0,0,0,0.1);
        }
        div.settings-btn-pressed {
            background-color: rgba(0,0,0,0.1);
        }
    };

    let window = web_sys::window().expect("no global `window` exists");
    let document = window.document().expect("should have a document on window");

    let closure = Closure::<dyn FnMut(_)>::new(move |event: Event| {
        let target = event.target().unwrap();
        let input_element = wasm_bindgen::JsCast::dyn_ref::<HtmlInputElement>(&target);
        if input_element.is_none() {
            event.prevent_default();
        }
    });

    document
        .add_event_listener_with_callback("contextmenu", closure.as_ref().unchecked_ref())
        .unwrap();
    closure.forget();

    mount_to_body(move || {
        view! { class=styler_class,
            <div class="settings-btn" class=("settings-btn-pressed", move || settings.get()) on:click=move |_| {
                set_settings.set(!settings.get());
            }>"Settings"</div>

            {
                move || if settings.get() {
                    view! {<Settings/>}
                } else {
                    view! {<App/>}
                }
            }
        }
    })
}
