use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex};

pub struct SharedState {
    pub power: bool,
}

#[derive(Clone, Copy)]
pub struct SharedStateMutex(pub &'static Mutex<CriticalSectionRawMutex, SharedState>);

pub struct AppState {
    pub shared: SharedStateMutex,
}
impl picoserve::extract::FromRef<AppState> for SharedStateMutex {
    fn from_ref(state: &AppState) -> Self {
        state.shared
    }
}
