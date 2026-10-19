//! A window with a header bar and one slider, the shape of the real vivid-gui main page.

use adw::prelude::*;

const APP_ID: &str = "dev.vividctl.Spike";

fn build(app: &adw::Application) {
    let scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, -1024.0, 1023.0, 1.0);
    scale.set_hexpand(true);
    scale.set_value(0.0);
    scale.connect_value_changed(|scale| println!("vibrance {}", scale.value()));

    let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
    content.set_margin_top(24);
    content.set_margin_bottom(24);
    content.set_margin_start(24);
    content.set_margin_end(24);
    content.append(&gtk::Label::new(Some("Digital Vibrance")));
    content.append(&scale);

    let view = adw::ToolbarView::new();
    view.add_top_bar(&adw::HeaderBar::new());
    view.set_content(Some(&content));

    adw::ApplicationWindow::builder()
        .application(app)
        .title("Vivid")
        .default_width(420)
        .content(&view)
        .build()
        .present();
}

fn main() -> gtk::glib::ExitCode {
    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_activate(build);
    app.run()
}
