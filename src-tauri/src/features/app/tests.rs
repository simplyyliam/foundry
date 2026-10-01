use std::sync::mpsc;
use std::time::Duration;

use tauri::test::{mock_builder, mock_context, noop_assets, MockRuntime, INVOKE_KEY};
use tauri::{App, Listener, WebviewWindowBuilder};

use super::*;

/// A mock app wired like the real one. Tauri only runs the setup hook from the event loop,
/// so tests mount events themselves and then call [`init`] (what setup calls).
fn mock_app() -> App<MockRuntime> {
    let app = crate::app_builder(mock_builder())
        .build(mock_context(noop_assets()))
        .unwrap();
    crate::specta_builder::<MockRuntime>().mount_events(&app);
    app
}

fn invoke(app: &App<MockRuntime>, cmd: &str) -> Result<serde_json::Value, serde_json::Value> {
    let webview = WebviewWindowBuilder::new(app, "main", Default::default())
        .build()
        .unwrap();
    tauri::test::get_ipc_response(
        &webview,
        tauri::webview::InvokeRequest {
            cmd: cmd.into(),
            callback: tauri::ipc::CallbackFn(0),
            error: tauri::ipc::CallbackFn(1),
            url: if cfg!(windows) {
                "http://tauri.localhost"
            } else {
                "tauri://localhost"
            }
            .parse()
            .unwrap(),
            body: tauri::ipc::InvokeBody::default(),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.to_string(),
        },
    )
    .map(|body| body.deserialize().unwrap())
}

#[test]
fn info_reports_version_and_os() {
    let info = info("1.2.3");
    assert_eq!(info.version, "1.2.3");
    assert_eq!(info.os, std::env::consts::OS);
}

#[test]
fn app_info_command_round_trips_over_ipc() {
    let app = mock_app();
    init(app.handle()).unwrap();
    let response = invoke(&app, "app_info").unwrap();
    let expected = info(app.package_info().version.to_string());
    assert_eq!(response, serde_json::to_value(expected).unwrap());
    assert_eq!(response["os"], std::env::consts::OS);
}

#[test]
fn init_emits_app_ready_with_id_and_ts() {
    let app = mock_app();
    let (tx, rx) = mpsc::channel();
    // Listen on the raw wire name to prove what the frontend will see.
    app.listen_any("app:ready", move |event| {
        tx.send(event.payload().to_string()).unwrap();
    });

    init(app.handle()).unwrap();

    let payload = rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let value: serde_json::Value = serde_json::from_str(&payload).unwrap();
    let object = value.as_object().unwrap();
    let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
    keys.sort();
    assert_eq!(keys, ["id", "ts", "version"]);
    assert_eq!(value["version"], app.package_info().version.to_string());

    let typed: AppReady = serde_json::from_value(value).unwrap();
    assert!(!typed.meta.id.is_empty());
}
