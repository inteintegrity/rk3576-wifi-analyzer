use std::time::{Instant, SystemTime, UNIX_EPOCH};
use flipctl_app::{font, paint::Surface, picture, theme::{color, PANEL_W, PANEL_H}, Key, Page, Text};
use crate::analyzer::{self, Band, Summary};
use crate::scanner::{AccessPoint, Scan};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Screen { #[default] List, Details, Channels }

pub struct Snapshot {
    pub interface: String,
    pub aps: Vec<AccessPoint>,
    pub summary: Summary,
    pub received: Instant,
    pub unix_time: u64,
}

#[derive(Default)]
pub struct ViewState {
    pub last: Option<Snapshot>,
    pub error: Option<String>,
    pub scanning: bool,
    pub screen: Screen,
    pub selected: usize,
    pub band5: bool,
    pub signal_chart: bool,
    pub channel_offset: usize,
    pub detail_offset: usize,
}

impl ViewState {
    pub fn finish(&mut self, result: Result<Scan, String>) {
        self.scanning = false;
        match result {
            Err(error) => { eprintln!("Wi-Fi scan error: {error}"); self.error = Some(error); }
            Ok(scan) => {
                let identity = self.ap().map(|a| a.bssid.clone());
                let (aps, summary) = analyzer::analyze(scan.aps);
                let found = identity.and_then(|id| aps.iter().position(|a| a.bssid == id));
                if self.screen == Screen::Details && found.is_none() { self.screen = Screen::List; }
                self.selected = found.unwrap_or(0);
                if aps.is_empty() { self.screen = Screen::List; }
                let unix_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
                eprintln!("Wi-Fi scan OK: {} APs; 2.4GHz {}; 5GHz {}; interface {}", aps.len(), summary.ghz24, summary.ghz5, scan.interface);
                let snapshot = Snapshot { interface: scan.interface, aps, summary, received: Instant::now(), unix_time };
                // Opt-in audit contains the exact nmcli output used by this UI update.
                // No audit files, fixtures or fake values are consumed by the application.
                if let Some(dir) = std::env::var_os("WIFI_ANALYZER_AUDIT_DIR") {
                    if let Err(e) = audit(std::path::Path::new(&dir), &scan.raw, &snapshot) {
                        eprintln!("Wi-Fi audit write: {e}");
                    }
                }
                self.last = Some(snapshot);
                self.error = None;
                self.detail_offset = 0;
            }
        }
    }

    pub fn ap(&self) -> Option<&AccessPoint> { self.last.as_ref()?.aps.get(self.selected) }
    pub fn count(&self) -> usize { self.last.as_ref().map_or(0, |s| s.aps.len()) }

    pub fn key(&mut self, key: Key) -> bool {
        match key {
            Key::View => {
                if self.screen == Screen::Channels { self.signal_chart = !self.signal_chart; }
                else { self.screen = Screen::Channels; }
            }
            Key::Ok => {
                self.screen = match self.screen { Screen::List if self.ap().is_some() => Screen::Details, _ => Screen::List };
                self.detail_offset = 0;
            }
            Key::Up | Key::Down | Key::Left | Key::Right => {
                let forward = matches!(key, Key::Down | Key::Right);
                match self.screen {
                    Screen::List => {
                        let n = self.count();
                        if n > 0 {
                            let step = if matches!(key, Key::Left | Key::Right) { 4 } else { 1 };
                            self.selected = if forward { (self.selected + step) % n } else { (self.selected + n - step % n) % n };
                        }
                    }
                    Screen::Details => {
                        if matches!(key, Key::Left | Key::Right) { self.screen = Screen::List; }
                        else if forward { self.detail_offset = (self.detail_offset + 1).min(self.detail_lines().len().saturating_sub(5)); }
                        else { self.detail_offset = self.detail_offset.saturating_sub(1); }
                    }
                    Screen::Channels => {
                        if matches!(key, Key::Up | Key::Down) { self.band5 = !self.band5; self.channel_offset = 0; }
                        else if forward {
                            let n = self.channels().len();
                            self.channel_offset = if self.channel_offset + 8 < n { self.channel_offset + 8 } else { 0 };
                        } else { self.channel_offset = self.channel_offset.saturating_sub(8); }
                    }
                }
            }
            _ => return false,
        }
        true
    }

    fn channels(&self) -> Vec<(u16, usize, u8)> {
        let Some(s) = &self.last else { return vec![] };
        let band = if self.band5 { Band::Ghz5 } else { Band::Ghz24 };
        let counts = if self.band5 { &s.summary.channels5 } else { &s.summary.channels24 };
        counts.iter().map(|(&ch,&count)| {
            let signal = s.aps.iter().filter(|a| Band::of(a) == band && a.channel == ch).map(|a| a.signal).max().unwrap_or(0);
            (ch,count,signal)
        }).collect()
    }

    fn detail_lines(&self) -> Vec<String> {
        let Some(ap) = self.ap() else { return vec!["No AP selected".into()] };
        let mut lines = wrap(&format!("SSID: {}", if ap.ssid.is_empty() { "<hidden>" } else { &ap.ssid }), 243);
        lines.push(format!("BSSID: {}", ap.bssid));
        lines.push(format!("Signal: {}%", ap.signal));
        lines.push(format!("Channel: {}  {}", ap.channel, Band::of(ap).label()));
        lines.push(format!("Frequency: {} MHz", ap.frequency_mhz));
        lines.extend(wrap(&format!("Security: {}", ap.security), 243));
        lines.push(format!("Connected: {}", if ap.connected { "yes" } else { "no" }));
        if let Some(s) = &self.last { lines.push(format!("Interface: {}", s.interface)); }
        if let Some(error) = &self.error { lines.extend(wrap(&format!("ERROR: {error}"), 243)); }
        lines
    }

    pub fn page(&self) -> Page {
        assert_eq!((PANEL_W, PANEL_H), (256,144));
        let mut surface = Surface::panel();
        let mut texts = Vec::new();
        let title = match self.screen { Screen::List => "Wi-Fi Network List", Screen::Details => "Network Details", Screen::Channels => "Channel Analysis" };
        label(&mut texts, 4, 14, title, false);
        let age = self.last.as_ref().map(|s| s.received.elapsed().as_secs());
        let status = if let Some(error) = &self.error {
            format!("ERROR: {}", if self.last.is_some() { "last good data retained" } else { error })
        } else if self.scanning {
            if self.last.is_some() { format!("Scanning... {} APs", self.count()) }
            else { "Scanning... no results yet".into() }
        } else if let Some(s) = &self.last {
            format!("{} APs  2.4G:{}  5G:{}  {}s", self.count(), s.summary.ghz24, s.summary.ghz5, age.unwrap_or(0))
        } else { "Waiting for first scan".into() };
        label(&mut texts, 4, 28, &fit(&status, 247), false);
        surface.hline(3,43,250,color::STATUS_DIM);
        match self.screen {
            Screen::List => {
                if let Some(s) = &self.last {
                    if s.aps.is_empty() { label(&mut texts, 5, 53, "No access points found", false); }
                    let start = (self.selected / 4) * 4;
                    for (i,ap) in s.aps.iter().enumerate().skip(start).take(4) {
                        let y = 46 + (i-start) as i32 * 16;
                        let chosen = i == self.selected;
                        if chosen { surface.round_rect(3,y,250,16,2,color::BLACK); }
                        let name = format!("{}{}", if ap.connected { "*" } else { "" }, if ap.ssid.is_empty() { "<hidden>" } else { &ap.ssid });
                        label(&mut texts, 6, y, &fit(&name, 151), chosen);
                        right(&mut texts, 205, y, &format!("{}%", ap.signal), chosen);
                        right(&mut texts, 250, y, &format!("c{}",ap.channel), chosen);
                    }
                    let info = if s.aps.is_empty() { "Run: scan again".into() }
                        else { format!("{}/{}  {}", self.selected+1,s.aps.len(),s.aps[self.selected].bssid) };
                    label(&mut texts, 5, 112, &info, false);
                } else {
                    let msg = self.error.as_deref().unwrap_or("Scanning nearby access points...");
                    for (i,line) in wrap(msg,244).iter().take(4).enumerate() { label(&mut texts,5,47+i as i32*16,line,false); }
                }
            }
            Screen::Details => {
                let lines = self.detail_lines();
                for (i,line) in lines.iter().skip(self.detail_offset).take(5).enumerate() { label(&mut texts,5,45+i as i32*13,line,false); }
                label(&mut texts,5,112,&format!("Rows {}-{} / {}  Up/Down",self.detail_offset+1,(self.detail_offset+5).min(lines.len()),lines.len()),false);
            }
            Screen::Channels => {
                let channels = self.channels();
                let start = self.channel_offset.min(channels.len().saturating_sub(1)) / 8 * 8;
                let shown: Vec<_> = channels.iter().skip(start).take(8).collect();
                let scale = if self.signal_chart { 100 } else { channels.iter().map(|c| c.1).max().unwrap_or(1).max(1) };
                label(&mut texts,5,45,&format!("{}  {}  max {}",if self.band5 {"5GHz"} else {"2.4GHz"},if self.signal_chart {"Signal %"} else {"AP count"},scale),false);
                surface.hline(8,98,240,color::BLACK);
                for (i,&&(ch,count,signal)) in shown.iter().enumerate() {
                    let x = 9 + i as i32 * 30;
                    let value = if self.signal_chart { signal as usize } else { count };
                    let height = (value * 24 / scale) as i32;
                    surface.rect(x+5,98-height,17,height,color::BLACK);
                    label(&mut texts,x+3,60,&value.to_string(),false);
                    label(&mut texts,x+2,98,&ch.to_string(),false);
                }
                if channels.is_empty() { label(&mut texts,8,78,"No APs in this band",false); }
                label(&mut texts,5,112,&format!("Up/Down: band  L/R: page {}/{}",start/8+1,channels.len().div_ceil(8).max(1)),false);
            }
        }
        Page::canvas(picture(&surface), texts)
    }
}

fn label(texts: &mut Vec<Text>, x: i32, y: i32, text: &str, white: bool) {
    texts.push(Text { x:x as f32,y:y as f32,text:text.into(),font:0,align:-1,white });
}
fn right(texts: &mut Vec<Text>, x: i32, y: i32, text: &str, white: bool) {
    texts.push(Text { x:x as f32,y:y as f32,text:text.into(),font:0,align:1,white });
}
fn fit(text: &str, width: i32) -> String {
    let clean: String = text.chars().map(|c| if c.is_control() {' '} else {c}).collect();
    if font::tw(&clean) <= width { return clean; }
    let mut result = String::new();
    for ch in clean.chars() {
        if font::tw(&format!("{result}{ch}...")) > width { break; }
        result.push(ch);
    }
    result + "..."
}
fn wrap(text: &str, width: i32) -> Vec<String> {
    let mut lines = Vec::new(); let mut line = String::new();
    for ch in text.chars().map(|c| if c.is_control() {' '} else {c}) {
        if !line.is_empty() && font::tw(&format!("{line}{ch}")) > width { lines.push(std::mem::take(&mut line)); }
        line.push(ch);
    }
    if !line.is_empty() { lines.push(line); }
    lines
}

fn audit(dir: &std::path::Path, raw: &str, s: &Snapshot) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let mut text = format!("time\t{}\ninterface\t{}\nAPs\t{}\n2.4GHz\t{}\n5GHz\t{}\nOther\t{}\nBSSID\tSignal%\tChannel\tFrequencyMHz\tConnected\n",
        s.unix_time,s.interface,s.aps.len(),s.summary.ghz24,s.summary.ghz5,s.summary.other);
    for a in &s.aps { text.push_str(&format!("{}\t{}\t{}\t{}\t{}\n",a.bssid,a.signal,a.channel,a.frequency_mhz,a.connected)); }
    // One immutable directory per scan. The manifest is written last.
    let scan_dir = dir.join(format!("scan-{}",s.unix_time));
    std::fs::create_dir_all(&scan_dir)?;
    std::fs::write(scan_dir.join("nmcli.txt"), raw)?;
    std::fs::write(scan_dir.join("ui.tsv"),text)?;
    let mut channels = "Band\tChannel\tAPs\tStrongestSignal%\n".to_string();
    for (band, counts) in [(Band::Ghz24, &s.summary.channels24), (Band::Ghz5, &s.summary.channels5)] {
        for (&channel, &count) in counts {
            let signal = s.aps.iter().filter(|a| Band::of(a) == band && a.channel == channel).map(|a| a.signal).max().unwrap_or(0);
            channels.push_str(&format!("{}\t{}\t{}\t{}\n", band.label(), channel, count, signal));
        }
    }
    std::fs::write(scan_dir.join("channels.tsv"), channels)?;
    std::fs::write(dir.join("latest"),scan_dir.to_string_lossy().as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn scan() -> Scan { Scan { interface:"wlan-test".into(), raw:String::new(), aps:crate::scanner::parse("Test:AA\\:BB\\:CC\\:DD\\:EE\\:01:6:2437 MHz:73:WPA2:*").unwrap() } }
    #[test] fn failure_keeps_last_success_and_success_clears_error() {
        let mut s = ViewState::default(); s.finish(Ok(scan())); s.finish(Err("radio unavailable".into()));
        assert_eq!(s.count(),1); assert_eq!(s.ap().unwrap().signal,73); assert!(s.error.is_some());
        s.finish(Ok(scan())); assert!(s.error.is_none());
    }
    #[test] fn empty_success_clears_old_results() {
        let mut s = ViewState::default(); s.finish(Ok(scan())); let mut empty = scan(); empty.aps.clear();
        s.finish(Ok(empty)); assert_eq!(s.count(),0); assert!(s.error.is_none());
    }
    #[test] fn navigation_and_chart_toggle() {
        let mut s=ViewState::default(); s.finish(Ok(scan())); s.key(Key::Ok); assert_eq!(s.screen,Screen::Details);
        s.key(Key::View); assert_eq!(s.screen,Screen::Channels); s.key(Key::View); assert!(s.signal_chart);
        s.key(Key::Down); assert!(s.band5); s.key(Key::Ok); assert_eq!(s.screen,Screen::List);
    }
    #[test] fn selection_tracks_bssid_after_reordering() {
        let mut s = ViewState::default(); s.finish(Ok(scan()));
        let mut next = scan(); let mut ap = next.aps[0].clone(); ap.bssid="AA:BB:CC:DD:EE:02".into(); ap.signal=99; next.aps.push(ap);
        s.finish(Ok(next)); assert_eq!(s.selected,1); assert_eq!(s.ap().unwrap().bssid,"AA:BB:CC:DD:EE:01");
    }
    #[test] fn disappearing_ap_returns_details_to_list() {
        let mut s = ViewState::default(); s.finish(Ok(scan())); s.screen = Screen::Details;
        let mut next = scan(); next.aps[0].bssid = "AA:BB:CC:DD:EE:02".into();
        s.finish(Ok(next)); assert_eq!(s.screen, Screen::List);
    }
}
