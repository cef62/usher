//! Install and the menu-bar item.

use tauri::{AppHandle, Manager};
use tauri_plugin_updater::Update;

use super::{BUSY, NEWER, NO_UPDATE, Phase, UpdateState, begin, bucket, pending, set_phase, state};
use crate::instance;

/// Why an install must not start: `None` when `available` is the version the user saw.
pub fn stale(available: Option<&str>, expected: &str) -> Option<&'static str> {
    match available {
        None => Some(NO_UPDATE),
        Some(v) if v != expected => Some(NEWER),
        Some(_) => None,
    }
}

/// Takes the stored update and the busy flag, or says why not.
fn claim(app: &AppHandle, expected: &str) -> Result<Update, String> {
    let mut st = state(app);
    if let Some(why) = stale(st.available.as_ref().map(|u| u.version.as_str()), expected) {
        return Err(why.into());
    }
    let update = st.available.clone().ok_or(NO_UPDATE)?;
    if !begin(&mut st) {
        return Err(BUSY.into());
    }
    set_phase(
        &mut st,
        Phase::Downloading {
            downloaded: 0,
            total: None,
        },
    );
    Ok(update)
}

async fn download(
    app: &AppHandle,
    update: &Update,
    dir: Option<&std::path::Path>,
) -> Result<(), String> {
    let (mut done, mut shown) = (0u64, None);
    let on_chunk = |len: usize, total: Option<u64>| {
        done += len as u64;
        let b = bucket(done, total, 100);
        if shown != Some(b) {
            shown = Some(b);
            set_phase(
                &mut state(app),
                Phase::Downloading {
                    downloaded: done,
                    total,
                },
            );
        }
    };
    // the plugin verifies the signature after this, then swaps the bundle. The mark makes the relaunch wait for this
    // process's instance lock; written after the download, so its ten minutes do not run out on a slow link
    let on_finish = || {
        set_phase(&mut state(app), Phase::Installing);
        if let Some(d) = dir {
            instance::mark_restart(d);
        }
    };
    update
        .download_and_install(on_chunk, on_finish)
        .await
        .map_err(|e| e.to_string())
}

fn fail(st: &mut UpdateState, dir: Option<&std::path::Path>, e: &str) {
    if let Some(d) = dir {
        instance::clear_restart_mark(d);
    }
    log::error!("update install failed: {e}");
    set_phase(
        st,
        Phase::Failed {
            message: format!("The update failed: {e}"),
        },
    );
}

/// Downloads and installs the stored update, then relaunches. `expected`: the version the user saw; a newer one found
/// since is refused, so it is never installed unseen.
pub async fn install(app: &AppHandle, expected: &str) -> Result<(), String> {
    let update = claim(app, expected)?;
    log::info!("installing usher {}", update.version);
    let dir = app.path().app_data_dir().ok();
    let result = download(app, &update, dir.as_deref()).await;
    let mut st = state(app);
    st.busy = false;
    if let Err(e) = result {
        fail(&mut st, dir.as_deref(), &e);
        return Err(e);
    }
    drop(st);
    // request_restart runs RunEvent::Exit (flushes state.json) before relaunching
    app.request_restart();
    Ok(())
}

/// The menu-bar item: "Check for Updates…", or "Update to usher X…" once one is found.
pub struct TrayItem(pub tauri::menu::MenuItem<tauri::Wry>);

pub fn tray_text(pending: Option<&str>) -> String {
    pending.map_or_else(
        || "Check for Updates…".into(),
        |v| format!("Update to usher {v}…"),
    )
}

pub fn refresh_tray(app: &AppHandle) {
    if let Some(item) = app.try_state::<TrayItem>() {
        item.0.set_text(tray_text(pending(app).as_deref())).ok();
    }
}

/// The menu-bar item. With an update found: the What's new window. Otherwise a check whose answer the user asked for:
/// What's new when one is found, else a message (up to date, busy or the error).
pub fn tray_clicked(app: &AppHandle) {
    if pending(app).is_some() {
        crate::whatsnew::open(app);
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
        let say = |kind, text: String| {
            app.dialog()
                .message(text)
                .title("usher")
                .kind(kind)
                .show(|_| {})
        };
        match super::check(&app).await {
            Ok(Some(_)) => {
                let a = app.clone();
                app.run_on_main_thread(move || crate::whatsnew::open(&a))
                    .ok();
            }
            Ok(None) => say(
                MessageDialogKind::Info,
                format!("usher {} is up to date.", app.package_info().version),
            ),
            Err(e) => say(
                MessageDialogKind::Warning,
                format!("Could not check for updates: {e}"),
            ),
        }
    });
}
