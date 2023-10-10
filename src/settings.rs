use leptos::*;
use stylers::style;

#[component]
pub fn Settings() -> impl IntoView {
    let styler_class = style! { "Settings",
      div.input-container {
        display: flex;
        flex-direction: column;
      }
      div.input-container input {
        margin: 10px 5px 5px 5px;
      }
    };

    let (token, set_token) = create_signal(String::new());
    let (server, set_server) = create_signal(String::new());

    let storage = window().local_storage().unwrap().unwrap();

    if let Ok(Some(value)) = storage.get_item("token") {
        set_token.set(value);
    }

    if let Ok(Some(value)) = storage.get_item("server") {
        set_server.set(value);
    }

    view! { class = styler_class,
        <div class="input-container">
          <input placeholder="TOKEN" type="password" on:input=move |ev| {
            let text = event_target_value(&ev);
            set_token.set(text.clone());
            let storage = window().local_storage().unwrap().unwrap();
            storage.set_item("token", text.as_str()).unwrap();
          } prop:value=token
          />
          <input placeholder="SERVER" on:input=move |ev| {
            let text = event_target_value(&ev);
            set_server.set(text.clone());
            let storage = window().local_storage().unwrap().unwrap();
            storage.set_item("server", text.as_str()).unwrap();
          } prop:value=server
          />
        </div>
    }
}
