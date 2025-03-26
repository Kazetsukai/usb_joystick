use embassy_net::Stack;
use embassy_time::Duration;
use picoserve::{
    extract::{self, State},
    make_static,
    response::{json, File, IntoResponse},
    routing::{get, get_service, post},
    AppRouter, AppWithStateBuilder, Config,
};

use crate::state::{AppState, SharedStateMutex};

const INDEX_HTML: &str = include_str!("../static/index.html");
const STYLE_CSS: &str = include_str!("../static/style.css");
const SCRIPT_JS: &str = include_str!("../static/script.js");

pub struct AppProps;

pub async fn get_state(
    extract::State(SharedStateMutex(shared)): extract::State<SharedStateMutex>,
) -> impl IntoResponse {
    json::Json((*shared.lock().await).power)
}

impl AppWithStateBuilder for AppProps {
    type State = AppState;
    type PathRouter = impl picoserve::routing::PathRouter<AppState>;

    fn build_app(self) -> picoserve::Router<Self::PathRouter, Self::State> {
        picoserve::Router::new()
            .route("/", get_service(File::html(INDEX_HTML)))
            .route("/style.css", get_service(File::css(STYLE_CSS)))
            .route("/script.js", get_service(File::javascript(SCRIPT_JS)))
            .route("/state", get(get_state))
            .route(
                "/power",
                post(
                    |State(SharedStateMutex(shared)): State<SharedStateMutex>| async {
                        let power = &mut shared.lock().await.power;
                        *power = !*power;
                        json::Json(*power)
                    },
                ),
            )
    }
}

pub fn make_web_app() -> (
    &'static AppRouter<AppProps>,
    &'static picoserve::Config<embassy_time::Duration>,
) {
    // Setup web app
    let app = make_static!(AppRouter<AppProps>, AppProps.build_app());
    let config = make_static!(
        picoserve::Config<embassy_time::Duration>,
        picoserve::Config::new(picoserve::Timeouts {
            start_read_request: Some(embassy_time::Duration::from_secs(5)),
            read_request: Some(embassy_time::Duration::from_secs(1)),
            write: Some(embassy_time::Duration::from_secs(1)),
        })
        .keep_connection_alive()
    );

    (app, config)
}

pub(crate) const WEB_TASK_POOL_SIZE: usize = 3;
#[embassy_executor::task(pool_size = WEB_TASK_POOL_SIZE)]
pub async fn web_task(
    id: usize,
    stack: Stack<'static>,
    state: AppState,
    app: &'static AppRouter<AppProps>,
    config: &'static Config<Duration>,
) -> ! {
    let port = 80;
    let mut tcp_rx_buffer = [0; 1024];
    let mut tcp_tx_buffer = [0; 1024];
    let mut http_buffer = [0; 2048];

    picoserve::listen_and_serve_with_state(
        id,
        app,
        config,
        stack,
        port,
        &mut tcp_rx_buffer,
        &mut tcp_tx_buffer,
        &mut http_buffer,
        &state,
    )
    .await
}
