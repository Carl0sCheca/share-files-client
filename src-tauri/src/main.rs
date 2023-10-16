// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use read_progress_stream::ReadProgressStream;
use std::{
    ffi::OsStr,
    io::Write,
    path::Path,
    process::{Command, Stdio},
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};
use tauri::{
    api::notification::Notification, CustomMenuItem, Manager, SystemTray, SystemTrayEvent,
    SystemTrayMenu, SystemTrayMenuItem,
};
use tokio_util::bytes::Bytes;

#[derive(Clone, serde::Serialize)]
struct Payload {
    args: Vec<String>,
    cwd: String,
}

#[derive(Debug)]
enum PasteType {
    Text,
    File,
    Screenshot,
}
#[derive(Debug)]
enum SendType {
    Text(String),
    File { data: Vec<u8>, format: String },
}

#[derive(serde::Deserialize, serde::Serialize)]
enum UploadResponse {
    Ok { message: String },
    Error { message: String },
}

impl SendType {
    fn get_type(&self) -> String {
        match self {
            SendType::Text(_) => "txt".to_owned(),
            SendType::File { format, .. } => format.to_owned(),
        }
    }

    fn get_bytes(&self) -> Vec<u8> {
        match self {
            SendType::Text(t) => t.as_bytes().to_vec(),
            SendType::File { data, .. } => data.clone(),
        }
    }
}

impl From<SendType> for reqwest::Body {
    fn from(value: SendType) -> Self {
        match value {
            SendType::Text(text) => Self::from(text),
            SendType::File { data, .. } => Self::from(data),
        }
    }
}

#[derive(Clone, serde::Serialize)]
struct ProgressPayload {
    progress: u64,
    total: u64,
    file_size: u64,
}

// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
#[tauri::command(rename_all = "snake_case")]
async fn upload_file(app: tauri::AppHandle, token: String, server_url: String) -> String {
    let mime_types = Command::new("wl-paste")
        .arg("-l")
        .output()
        .expect("Failed to execute command");

    let mime_types = String::from_utf8_lossy(&mime_types.stdout)
        .split('\n')
        .filter(|t| !t.is_empty())
        .map(|s| s.to_owned())
        .collect::<Vec<String>>();

    let pasted_type = match mime_types
        .iter()
        .find(|x| x.starts_with("image/") || x.starts_with("text/"))
    {
        Some(x) if x.starts_with("image/") => PasteType::Screenshot,
        Some(x) if x.starts_with("text/uri-list") => PasteType::File,
        Some(x) if x.starts_with("text/plain") => PasteType::Text,
        _ => PasteType::Text,
    };

    let output = Command::new("wl-paste")
        .output()
        .expect("Failed to execute command");

    let file = match pasted_type {
        PasteType::Text => SendType::Text(String::from_utf8_lossy(&output.stdout).to_string()),
        PasteType::File => {
            let file_path = String::from_utf8(output.stdout).unwrap();
            let file_path = file_path.replace("file://", "");

            // TODO: upload multiple files?
            let file_paths = file_path
                .split("\r")
                .map(|s| s.to_owned().replace("\n", "").replace("%20", " "))
                .filter(|s| !s.is_empty())
                .collect::<Vec<String>>();

            let file_path = file_paths[0].clone();
            let path = Path::new(&file_path);
            let format = path
                .extension()
                .unwrap_or(&OsStr::new(""))
                .to_str()
                .unwrap()
                .to_string();
            let data = std::fs::read(path).expect("Cannot read the file");
            SendType::File { data, format }
        }
        PasteType::Screenshot => SendType::File {
            data: output.stdout,
            format: "png".to_owned(),
        },
    };

    let window = app.get_window("main").unwrap();

    let client = reqwest::Client::new();

    let chunks: Vec<Result<_, ::std::io::Error>> = file
        .get_bytes()
        .chunks(1024)
        .map(|f| Ok(Bytes::from(Vec::from(f))))
        .collect();

    let file_size = file.get_bytes().len() as u64;

    let stream = futures_util::stream::iter(chunks);

    let last_emit = Arc::new(RwLock::new(Instant::now()));

    let body = reqwest::Body::wrap_stream(ReadProgressStream::new(
        stream,
        Box::new(move |progress, total| {
            let now = Instant::now();
            let should_emit = {
                let mut ea = last_emit.write().unwrap();
                if total == file_size || now.duration_since(*ea) >= Duration::from_millis(50) {
                    *ea = now;
                    true
                } else {
                    false
                }
            };
            if should_emit {
                let total = if file_size == total {
                    file_size - 1
                } else {
                    total
                };
                window
                    .emit(
                        "upload://progress",
                        ProgressPayload {
                            progress,
                            total,
                            file_size,
                        },
                    )
                    .unwrap();
            }
        }),
    ));

    match client
        .post(format!("{server_url}/upload"))
        .header("format", file.get_type())
        .header("share-token", token)
        .body(body)
        .send()
        .await
    {
        Ok(res) => {
            let response_text = res.text().await.unwrap();

            let upload_response: UploadResponse =
                serde_json::from_str(response_text.as_str()).unwrap();

            if let UploadResponse::Error { .. } = upload_response {
                return format!("{}", response_text);
            }

            let mut output = Command::new("wl-copy")
                .stdin(Stdio::piped())
                .spawn()
                .unwrap();

            let child_stdin = output.stdin.as_mut().unwrap();

            if let UploadResponse::Ok { message } = upload_response {
                child_stdin.write_all(message.as_bytes()).unwrap();
            }

            _ = output.wait_with_output().unwrap();

            let window = app.get_window("main").unwrap();
            window
                .emit(
                    "upload://progress",
                    ProgressPayload {
                        progress: 1u64,
                        total: file_size,
                        file_size,
                    },
                )
                .unwrap();
            let notification = Notification::new(&app.config().tauri.bundle.identifier)
                .title("Done")
                .body("The file has been upload");
            notification.show().unwrap();

            format!("{}", response_text)
        }
        Err(error) => serde_json::to_string(&UploadResponse::Error {
            message: error.to_string(),
        })
        .unwrap(),
    }
}

fn main() {
    let quit = CustomMenuItem::new("quit".to_string(), "Quit");
    let hide = CustomMenuItem::new("hide".to_string(), "Show/Hide");
    let upload = CustomMenuItem::new("upload".to_string(), "Upload");
    let tray_menu = SystemTrayMenu::new()
        .add_item(quit)
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(hide)
        .add_item(upload);

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            app.app_handle().get_window("main").unwrap().show().unwrap();
            app.app_handle()
                .get_window("main")
                .unwrap()
                .set_focus()
                .unwrap();
        }))
        .on_window_event(|event| match event.event() {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                event.window().hide().unwrap();
                api.prevent_close();
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![upload_file])
        .setup(|app| {
            let app_handle = app.handle();
            SystemTray::new()
                .with_menu(tray_menu)
                .on_event(move |event| match event {
                    SystemTrayEvent::MenuItemClick { id, .. } => match id.as_str() {
                        "quit" => {
                            std::process::exit(0);
                        }
                        "hide" => {
                            let window = app_handle.get_window("main").unwrap();
                            match window.is_visible().unwrap() {
                                true => window.hide().unwrap(),
                                false => window.show().unwrap(),
                            }
                        }
                        "upload" => {
                            let window = app_handle.get_window("main").unwrap();
                            window.emit("upload://start", "go").unwrap();
                        }
                        _ => {}
                    },
                    _ => {}
                })
                .build(app)
                .unwrap();
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|_app_handle, event| match event {
            tauri::RunEvent::ExitRequested { api, .. } => {
                api.prevent_exit();
            }
            _ => {}
        });
}
