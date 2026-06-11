use std::cell::Cell;
use std::rc::Rc;

use async_channel::{Receiver, Sender};
use gtk4::prelude::*;
use libadwaita::prelude::*;

use crate::spotify::{Command, Event};
use crate::ui::{NowPlayingPanel, RecentsPanel, TrackListPanel};

/// Builds the main window: now-playing/turntable on the left, search
/// results in the centre, recently played tracks on the right, and a
/// search bar docked at the bottom centre.
pub fn build_window(
    app: &libadwaita::Application,
    cmd_tx: Sender<Command>,
    evt_rx: Receiver<Event>,
) -> libadwaita::ApplicationWindow {
    load_css();

    let window = libadwaita::ApplicationWindow::builder()
        .application(app)
        .title("Velo Player")
        .default_width(1440)
        .default_height(900)
        .build();

    // ── Header bar ──
    let brand = gtk4::Label::builder().label("VELO").css_classes(vec!["brand-mark"]).build();
    let title = gtk4::Label::builder().label("Player").css_classes(vec!["header-title"]).build();
    let status_label = gtk4::Label::builder()
        .label("Connecting…")
        .css_classes(vec!["status-label"])
        .ellipsize(gtk4::pango::EllipsizeMode::End)
        .build();

    let menu_btn = gtk4::MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .tooltip_text("Menu")
        .build();
    let menu = gio::Menu::new();
    menu.append(Some("Check for Updates…"), Some("win.check-update"));
    menu_btn.set_menu_model(Some(&menu));

    let check_update_action = gio::SimpleAction::new("check-update", None);
    check_update_action.connect_activate(glib::clone!(
        #[weak]
        window,
        move |_, _| check_for_updates(&window)
    ));
    window.add_action(&check_update_action);

    let header = libadwaita::HeaderBar::new();
    header.pack_start(&brand);
    header.set_title_widget(Some(&title));
    header.pack_end(&menu_btn);
    header.pack_end(&status_label);

    // ── Banner for setup / connection issues ──
    let banner = libadwaita::Banner::new("");
    banner.set_button_label(Some("Retry"));
    banner.connect_button_clicked(glib::clone!(
        #[strong]
        cmd_tx,
        move |_| {
            let _ = cmd_tx.send_blocking(Command::Login);
        }
    ));

    // One-click link to the Spotify Developer Dashboard's "create app" page,
    // shown alongside the banner while setup/login isn't complete yet.
    let dashboard_link = gtk4::LinkButton::builder()
        .label("Open Spotify Dashboard to add your Client ID ↗")
        .uri("https://developer.spotify.com/dashboard/create")
        .css_classes(vec!["dashboard-link"])
        .halign(gtk4::Align::Center)
        .visible(false)
        .build();

    // ── Panels ──
    let now_playing = NowPlayingPanel::build(cmd_tx.clone());
    let track_list = TrackListPanel::build(cmd_tx.clone());
    let recents = RecentsPanel::build(cmd_tx.clone());

    let main_box = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .css_classes(vec!["velo-main"])
        .vexpand(true)
        .build();
    main_box.append(&now_playing.root);
    main_box.append(&track_list.root);
    main_box.append(&recents.root);

    // ── Search bar, bottom centre ──
    let search_entry = gtk4::SearchEntry::builder()
        .placeholder_text("Search tracks, artists, albums…")
        .css_classes(vec!["search-bar"])
        .width_request(420)
        .build();

    search_entry.connect_activate(glib::clone!(
        #[strong]
        cmd_tx,
        move |entry| {
            let query = entry.text().to_string();
            if !query.trim().is_empty() {
                let _ = cmd_tx.send_blocking(Command::Search(query));
            }
        }
    ));

    let search_row = gtk4::CenterBox::builder().css_classes(vec!["search-bar-row"]).build();
    search_row.set_center_widget(Some(&search_entry));

    let content_box = gtk4::Box::builder().orientation(gtk4::Orientation::Vertical).build();
    content_box.append(&banner);
    content_box.append(&dashboard_link);
    content_box.append(&main_box);
    content_box.append(&search_row);

    let toast_overlay = libadwaita::ToastOverlay::new();
    toast_overlay.set_child(Some(&content_box));

    let toolbar_view = libadwaita::ToolbarView::new();
    toolbar_view.add_top_bar(&header);
    toolbar_view.set_content(Some(&toast_overlay));

    window.set_content(Some(&toolbar_view));

    // ── Wire up controller events ──
    // Tracks whether we've successfully logged in, so errors after that
    // point show as toasts rather than taking over the banner.
    let connected = Rc::new(Cell::new(false));

    glib::MainContext::default().spawn_local(async move {
        while let Ok(event) = evt_rx.recv().await {
            match event {
                Event::Status(message) => status_label.set_label(&message),

                Event::ConfigRequired(path) => {
                    banner.set_title(&glib::markup_escape_text(&format!(
                        "Add your Spotify Client ID to {} and click Retry",
                        path.display()
                    )));
                    banner.set_revealed(true);
                    dashboard_link.set_visible(true);
                    status_label.set_label("Setup required");
                }

                Event::LoggedIn { display_name } => {
                    connected.set(true);
                    banner.set_revealed(false);
                    dashboard_link.set_visible(false);
                    status_label.set_label(&format!("Connected as {display_name}"));
                }

                Event::Error(message) => {
                    let escaped = glib::markup_escape_text(&message);
                    if connected.get() {
                        toast_overlay.add_toast(libadwaita::Toast::new(&escaped));
                    } else {
                        banner.set_title(&escaped);
                        banner.set_revealed(true);
                        dashboard_link.set_visible(true);
                        status_label.set_label("Connection error");
                    }
                }

                Event::Recents(tracks) => recents.set_recents(tracks),

                Event::SearchResults(tracks) => track_list.show_tracks("Search results", tracks),

                Event::NowPlaying(track) => {
                    let uri = track.as_ref().map(|t| t.uri.clone());
                    track_list.set_now_playing_uri(uri.clone());
                    recents.set_now_playing_uri(uri);
                    now_playing.set_now_playing(track.as_ref());
                }

                Event::PlaybackStatus(status) => now_playing.set_playback_status(status),

                Event::Position { position_ms, duration_ms } => now_playing.set_position(position_ms, duration_ms),

                Event::Volume(volume) => now_playing.set_volume(volume),
            }
        }
    });

    window
}

fn load_css() {
    let provider = gtk4::CssProvider::new();
    provider.load_from_string(include_str!("style.css"));
    if let Some(display) = gtk4::gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

// ── Self-update ──────────────────────────────────────────────────────────────

fn check_for_updates(window: &libadwaita::ApplicationWindow) {
    crate::update::check_for_update(glib::clone!(
        #[weak]
        window,
        #[upgrade_or]
        (),
        move |result| match result {
            crate::update::CheckResult::UpToDate => show_alert(
                &window,
                "Velo Player is up to date",
                &format!("You're running the latest version (commit {}).", crate::update::CURRENT_COMMIT),
            ),
            crate::update::CheckResult::Available { local, remote } => {
                let dialog = gtk4::AlertDialog::builder()
                    .modal(true)
                    .message("Update available")
                    .detail(format!(
                        "Velo Player {local} → {remote} is available.\n\n\
                         Pull, build, and install the update now? \
                         You may be asked for your password to finish installing."
                    ))
                    .buttons(["Later", "Update Now"])
                    .cancel_button(0)
                    .default_button(1)
                    .build();

                dialog.choose(Some(&window), gtk4::gio::Cancellable::NONE, glib::clone!(
                    #[weak]
                    window,
                    #[upgrade_or]
                    (),
                    move |response| {
                        if matches!(response, Ok(1)) {
                            start_update(&window);
                        }
                    }
                ));
            }
            crate::update::CheckResult::Unavailable(msg) => {
                show_alert(&window, "Can't check for updates", &msg)
            }
        }
    ));
}

fn start_update(window: &libadwaita::ApplicationWindow) {
    let progress = progress_dialog(
        window,
        "Updating Velo Player…",
        "Pulling, building, and installing the latest version.\nThis may take a few minutes.",
    );

    crate::update::run_update(glib::clone!(
        #[weak]
        window,
        #[strong]
        progress,
        #[upgrade_or]
        (),
        move |result| {
            progress.close();
            match result {
                crate::update::UpdateResult::Success => {
                    let dialog = gtk4::AlertDialog::builder()
                        .modal(true)
                        .message("Update complete")
                        .detail("Velo Player has been updated. Restart now to use the new version?")
                        .buttons(["Later", "Restart Now"])
                        .cancel_button(0)
                        .default_button(1)
                        .build();

                    dialog.choose(Some(&window), gtk4::gio::Cancellable::NONE, |response| {
                        if matches!(response, Ok(1)) {
                            crate::update::restart();
                        }
                    });
                }
                crate::update::UpdateResult::Failed(msg) => {
                    show_alert(&window, "Update failed", &msg)
                }
            }
        }
    ));
}

fn show_alert(window: &libadwaita::ApplicationWindow, message: &str, detail: &str) {
    gtk4::AlertDialog::builder()
        .modal(true)
        .message(message)
        .detail(detail)
        .buttons(["OK"])
        .build()
        .show(Some(window));
}

/// A small modal "working" dialog with a spinner, shown while an update runs
/// in the background. Caller closes it via the returned handle.
fn progress_dialog(parent: &libadwaita::ApplicationWindow, title: &str, body: &str) -> gtk4::Window {
    let spinner = gtk4::Spinner::builder()
        .spinning(true)
        .width_request(32)
        .height_request(32)
        .halign(gtk4::Align::Center)
        .build();

    let title_lbl = gtk4::Label::builder()
        .label(title)
        .css_classes(vec!["title-4"])
        .build();

    let body_lbl = gtk4::Label::builder()
        .label(body)
        .wrap(true)
        .justify(gtk4::Justification::Center)
        .css_classes(vec!["dim-label"])
        .build();

    let content = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .spacing(14)
        .margin_top(28)
        .margin_bottom(28)
        .margin_start(28)
        .margin_end(28)
        .build();
    content.append(&spinner);
    content.append(&title_lbl);
    content.append(&body_lbl);

    let win = gtk4::Window::builder()
        .transient_for(parent)
        .modal(true)
        .resizable(false)
        .deletable(false)
        .destroy_with_parent(true)
        .build();
    win.set_child(Some(&content));
    win.present();
    win
}
