use gtk4::prelude::*;

mod spotify;
mod ui;
mod update;
mod window;

use spotify::{controller, Command, Event};

fn main() -> glib::ExitCode {
    env_logger::init();

    let app = libadwaita::Application::builder().application_id("com.velo.Player").build();

    app.connect_activate(|app| {
        libadwaita::StyleManager::default().set_color_scheme(libadwaita::ColorScheme::ForceDark);

        let (cmd_tx, cmd_rx) = async_channel::unbounded::<Command>();
        let (evt_tx, evt_rx) = async_channel::unbounded::<Event>();

        controller::spawn(cmd_rx, evt_tx);

        window::build_window(app, cmd_tx, evt_rx).present();
    });

    app.run()
}
