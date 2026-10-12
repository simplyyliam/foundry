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

mod window_radius_detection {
    use super::super::window_radius::{macos_radius, windhawk_radius, RegValue};

    const ENABLED: Option<RegValue> = Some(RegValue::Dword(0));

    #[test]
    fn valid_radius_is_returned() {
        assert_eq!(
            windhawk_radius(ENABLED, Some(RegValue::Dword(26))),
            Some(26)
        );
        assert_eq!(windhawk_radius(ENABLED, Some(RegValue::Dword(1))), Some(1));
        assert_eq!(
            windhawk_radius(ENABLED, Some(RegValue::Dword(64))),
            Some(64)
        );
    }

    #[test]
    fn missing_disabled_value_counts_as_enabled() {
        assert_eq!(windhawk_radius(None, Some(RegValue::Dword(26))), Some(26));
    }

    #[test]
    fn disabled_mod_gives_none() {
        assert_eq!(
            windhawk_radius(Some(RegValue::Dword(1)), Some(RegValue::Dword(26))),
            None
        );
        assert_eq!(
            windhawk_radius(Some(RegValue::NotDword), Some(RegValue::Dword(26))),
            None
        );
    }

    #[test]
    fn missing_key_gives_none() {
        assert_eq!(windhawk_radius(None, None), None);
        assert_eq!(windhawk_radius(ENABLED, None), None);
    }

    #[test]
    fn non_numeric_radius_gives_none() {
        assert_eq!(windhawk_radius(ENABLED, Some(RegValue::NotDword)), None);
    }

    #[test]
    fn out_of_range_radius_gives_none() {
        assert_eq!(windhawk_radius(ENABLED, Some(RegValue::Dword(0))), None);
        assert_eq!(windhawk_radius(ENABLED, Some(RegValue::Dword(65))), None);
        assert_eq!(
            windhawk_radius(ENABLED, Some(RegValue::Dword(u32::MAX))),
            None
        );
    }

    #[test]
    fn macos_26_and_later_gives_26() {
        assert_eq!(macos_radius("26.0"), Some(26));
        assert_eq!(
            macos_radius(
                "26.1
"
            ),
            Some(26)
        );
        assert_eq!(macos_radius("27"), Some(26));
    }

    #[test]
    fn older_macos_gives_none() {
        assert_eq!(macos_radius("15.4"), None);
        assert_eq!(macos_radius("10.15.7"), None);
    }

    #[test]
    fn garbage_macos_version_gives_none() {
        assert_eq!(macos_radius(""), None);
        assert_eq!(macos_radius("abc"), None);
        assert_eq!(macos_radius("-26.0"), None);
        assert_eq!(macos_radius(".26"), None);
    }
}

#[test]
fn get_window_radius_command_round_trips_over_ipc() {
    let app = mock_app();
    let response = invoke(&app, "get_window_radius").unwrap();
    let object = response.as_object().unwrap();
    assert_eq!(object.keys().collect::<Vec<_>>(), ["radius"]);
    let radius = &response["radius"];
    assert!(
        radius.is_null() || (1..=64).contains(&radius.as_u64().unwrap()),
        "unexpected radius {radius}"
    );
}
