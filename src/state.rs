use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex};

#[derive(Clone, Copy)]
pub struct SharedState(pub &'static Mutex<CriticalSectionRawMutex, bool>);

pub struct AppState {
    pub power_state: SharedState,
}
impl picoserve::extract::FromRef<AppState> for SharedState {
    fn from_ref(state: &AppState) -> Self {
        state.power_state
    }
}
