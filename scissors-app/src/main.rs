use scissors_app::{
    app::{init_app, open_scissors_window},
    app_info::SCISSORS_APP_NAME,
    assets::{self, ScissorsAssets},
};

fn main() {
    gpui_platform::application()
        .with_assets(ScissorsAssets)
        .run(|cx| {
            assets::load_scissors_fonts(cx).expect("failed to load Scissors fonts");
            open_scissors_window(cx);
            init_app(cx, SCISSORS_APP_NAME);
        });
}
