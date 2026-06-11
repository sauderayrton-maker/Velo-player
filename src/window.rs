use std::cell::Cell;
use std::rc::Rc;

use async_channel::{Receiver, Sender};
use gtk4::prelude::*;
use libadwaita::prelude::*;

use crate::spotify::{Command, Event};
use crate::ui::{NowPlayingPanel, PlaylistsPanel, TrackListPanel};

/// Builds the main window: now-playing/turntable on the left, the track
/// list (search results or a playlist) in the centre, the playlist sidebar
/// on the right, and a search bar docked at the bottom centre.
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

    let header = libadwaita::HeaderBar::new();
    header.pack_start(&brand);
    header.set_title_widget(Some(&title));
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

    // ── Panels ──
    let now_playing = NowPlayingPanel::build(cmd_tx.clone());
    let track_list = TrackListPanel::build(cmd_tx.clone());
    let playlists = PlaylistsPanel::build(cmd_tx.clone());

    let main_box = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .css_classes(vec!["velo-main"])
        .vexpand(true)
        .build();
    main_box.append(&now_playing.root);
    main_box.append(&track_list.root);
    main_box.append(&playlists.root);

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
                    banner.set_title(&format!(
                        "Add your Spotify Client ID to {} and click Retry",
                        path.display()
                    ));
                    banner.set_revealed(true);
                    status_label.set_label("Setup required");
                }

                Event::LoggedIn { display_name } => {
                    connected.set(true);
                    banner.set_revealed(false);
                    status_label.set_label(&format!("Connected as {display_name}"));
                }

                Event::Error(message) => {
                    if connected.get() {
                        toast_overlay.add_toast(libadwaita::Toast::new(&message));
                    } else {
                        banner.set_title(&message);
                        banner.set_revealed(true);
                        status_label.set_label("Connection error");
                    }
                }

                Event::Playlists(playlists_list) => playlists.set_playlists(playlists_list),

                Event::PlaylistTracks { playlist, tracks } => track_list.show_tracks(&playlist.name, tracks),

                Event::SearchResults(tracks) => track_list.show_tracks("Search results", tracks),

                Event::NowPlaying(track) => {
                    track_list.set_now_playing_uri(track.as_ref().map(|t| t.uri.clone()));
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
