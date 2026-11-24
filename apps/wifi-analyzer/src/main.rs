mod scanner;
mod analyzer;
mod ui;

use std::time::{Duration, Instant};
use flipctl_app::{App, Key, Page, Ui};
use scanner::{Event, Worker};
use ui::{Screen, ViewState};

struct WifiAnalyzer {
    state: ViewState,
    worker: Option<Worker>,
    run_guard: RunGuard,
}

/// App::key receives key-down repeats but no key-up. A held remote Run key must
/// trigger only once, including when a scan finishes while it is still held.
/// Every repeat extends the quiet period, even while the scanner is busy.
#[derive(Default)]
struct RunGuard { last_event: Option<Instant> }
impl RunGuard {
    fn fresh_press(&mut self, now: Instant) -> bool {
        self.last_event.replace(now).is_none_or(|last| now.saturating_duration_since(last) >= Duration::from_secs(2))
    }
}
impl WifiAnalyzer {
    fn new() -> Self { Self { state:ViewState { scanning:true,..ViewState::default() }, worker:Some(Worker::start()), run_guard:RunGuard::default() } }
}
impl App for WifiAnalyzer {
    fn page(&self) -> Page { self.state.page() }
    fn buttons(&self) -> [&str;5] {
        ["Close", if self.state.screen == Screen::Channels { "Metric" } else { "Chart" }, "", "", if self.state.scanning { "Busy" } else { "Scan" }]
    }
    fn key(&mut self, key:Key, _: &mut Ui) -> bool {
        if key == Key::Run {
            if self.run_guard.fresh_press(Instant::now()) && !self.state.scanning {
                if let Some(worker) = &self.worker { worker.refresh(); self.state.scanning=true; }
            }
            return true;
        }
        self.state.key(key)
    }
    // The pinned RK3576 host's capture deadline is 120ms. Replace the canvas at
    // 20Hz so its damage-waiting capture stays active instead of timing out.
    // This clock only drains messages and updates scan age; it NEVER invokes nmcli.
    fn every(&self) -> Option<Duration> { Some(Duration::from_millis(50)) }
    fn tick(&mut self, _: &mut Ui) {
        if let Some(worker) = &self.worker {
            while let Ok(event) = worker.events.try_recv() {
                match event { Event::Started => self.state.scanning=true, Event::Finished(result) => self.state.finish(result) }
            }
        }
    }
    fn close(&mut self) { self.worker.take(); }
}
flipctl_app::main!(WifiAnalyzer::new());

#[cfg(test)]
mod render_tests {
    use super::*;
    #[test]
    fn holding_run_does_not_rescan_when_a_scan_finishes() {
        let start = Instant::now();
        let mut guard = RunGuard::default();
        assert!(guard.fresh_press(start));
        // Typical keyboard repeat begins after 600ms and continues at 25Hz.
        // Keep holding through several possible scan completions.
        for ms in (600..10_000).step_by(40) {
            assert!(!guard.fresh_press(start + Duration::from_millis(ms)));
        }
        assert!(guard.fresh_press(start + Duration::from_secs(12)));
    }
    /// Explicit opt-in integration test. Its only input is a real hardware scan.
    #[test]
    #[ignore = "requires a usable NetworkManager Wi-Fi interface"]
    fn real_scan_and_render() {
        let scan = scanner::scan().expect("real hardware scan");
        let mut state = ViewState::default(); state.finish(Ok(scan));
        assert!(state.error.is_none());
        let allowed:Vec<u8> = flipctl_app::theme::ALL_COLORS.iter().map(|(_,_,r,..)| *r).collect();
        for (name,screen,band5,signal_chart) in [
            ("list",Screen::List,false,false),("details",Screen::Details,false,false),
            ("channels24",Screen::Channels,false,false),("channels5",Screen::Channels,true,false),
            ("signal5",Screen::Channels,true,true)] {
            state.screen=screen; state.band5=band5; state.signal_chart=signal_chart;
            struct Draw<'a>(&'a ViewState);
            impl App for Draw<'_> {
                fn page(&self) -> Page { self.0.page() }
                fn buttons(&self) -> [&str;5] { ["Close","Chart","","","Scan"] }
            }
            let frame=flipctl_pages::shot(Draw(&state),flipctl_app::Status::default());
            assert_eq!(frame.len(),256*144);
            assert!(frame.iter().all(|p| allowed.contains(p)),"off-palette pixels");
            flipctl_app::render::save(&std::path::Path::new("target/wifi-verification").join(format!("{name}.png")),&frame);
        }
    }
}
