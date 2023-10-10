use js_sys::Function;
use leptos::leptos_dom::ev::SubmitEvent;
use leptos::*;
use serde::{Deserialize, Serialize};
use serde_wasm_bindgen::to_value;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "tauri"])]
    async fn invoke(cmd: &str, args: JsValue) -> JsValue;
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "event"])]
    async fn listen(event: &str, handler: &Function) -> JsValue;
}

#[derive(Serialize, Deserialize)]
struct SubmitArgs {
    token: String,
    server_url: String,
}

#[derive(Debug, Deserialize)]
struct ProgressPayload {
    #[allow(dead_code)]
    progress: u64,
    total: u64,
    file_size: u64,
}

#[derive(Deserialize)]
enum UploadResponse {
    Ok { message: String },
    Error { message: String },
}

#[component]
pub fn App() -> impl IntoView {
    let (url_file, set_url_file) = create_signal(String::new());
    let (progress_download, set_progress_download) = create_signal(0u64);
    let (error_message, set_error_message) = create_signal(String::new());

    let send_file = move |ev: SubmitEvent| {
        ev.prevent_default();
        set_url_file.set("".to_owned());
        set_progress_download.set(0);
        set_error_message.set("".to_owned());

        let storage = window().local_storage().unwrap().unwrap();

        let token = match storage.get_item("token") {
            Ok(Some(value)) if !value.is_empty() => value,
            _ => {
                set_error_message.set("Token is empty. Check settings".to_owned());
                return;
            }
        };

        let server_url = match storage.get_item("server") {
            Ok(Some(value)) if !value.is_empty() => value,
            _ => {
                set_error_message.set("Server URL is empty. Check settings".to_owned());
                return;
            }
        };

        spawn_local(async move {
            let args = to_value(&SubmitArgs { token, server_url }).unwrap();
            let new_msg = invoke("upload_file", args).await.as_string().unwrap();

            let response: UploadResponse = serde_json::from_str(new_msg.as_str()).unwrap();

            match response {
                UploadResponse::Ok { message } => set_url_file.set(message),
                UploadResponse::Error { message } => set_error_message.set(message),
            }
        });
    };

    spawn_local(async move {
        let closure = Closure::<dyn FnMut(_)>::new(move |event: JsValue| {
            let payload: serde_json::Value = serde_wasm_bindgen::from_value(event).unwrap();
            let payload: ProgressPayload =
                serde_json::from_value(payload["payload"].clone()).unwrap();

            let total_downloaded = (payload.total as f64 / payload.file_size as f64 * 100.0) as u64;

            set_progress_download.set(total_downloaded);
        });

        listen("upload://progress", closure.as_ref().unchecked_ref()).await;
        closure.forget();

        let closure = Closure::<dyn FnMut(_)>::new(move |_: JsValue| {
            send_file(SubmitEvent::new("").unwrap());
        });
        listen("upload://start", closure.as_ref().unchecked_ref()).await;
        closure.forget();
    });

    view! {
        <main class="container">
            <form class="row" on:submit=send_file>
                <button type="submit">"Upload file"</button>
            </form>
            {
                move || if !error_message.get().is_empty() {
                    view! {
                        <p style="color:red">{move || error_message}</p>
                    }
                } else if url_file.get().is_empty() && progress_download.get() != 0 {
                    view! {
                        <p>{ move || progress_download.get() }"%"</p>
                    }
                } else {
                    view! {
                        <p><b><a target="_blank" href={ move || url_file.get() }>{ move || url_file.get() }</a></b></p>
                    }
                }
            }
        </main>
    }
}
