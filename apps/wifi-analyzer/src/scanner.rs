//! Read-only NetworkManager scans. Never invoke a shell or change radio/configuration.
use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::thread;
use std::time::{Duration, Instant};

pub const REFRESH: Duration = Duration::from_secs(25);
const LIMIT: Duration = Duration::from_secs(22);
const FIELDS: &str = "SSID,BSSID,CHAN,FREQ,SIGNAL,SECURITY,IN-USE";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccessPoint {
    pub ssid: String,
    pub bssid: String,
    pub channel: u16,
    pub frequency_mhz: u16,
    pub signal: u8,
    pub security: String,
    pub connected: bool,
}

#[derive(Clone, Debug)]
pub struct Scan {
    pub interface: String,
    pub raw: String,
    pub aps: Vec<AccessPoint>,
}

/// nmcli -t -e yes escapes both ':' and '\\'. Reject incomplete/unknown escapes.
pub fn fields(line: &str) -> Result<Vec<String>, String> {
    let mut result = vec![String::new()];
    let mut escaped = false;
    for ch in line.chars() {
        if escaped {
            if ch != ':' && ch != '\\' {
                return Err("Invalid nmcli escape".into());
            }
            result.last_mut().unwrap().push(ch);
            escaped = false;
        } else {
            match ch {
                '\\' => escaped = true,
                ':' => result.push(String::new()),
                _ => result.last_mut().unwrap().push(ch),
            }
        }
    }
    if escaped { return Err("Incomplete nmcli escape".into()); }
    Ok(result)
}

pub fn parse(raw: &str) -> Result<Vec<AccessPoint>, String> {
    let mut aps = Vec::new();
    for (line_no, line) in raw.lines().enumerate() {
        // A genuinely empty result is valid; blank records within output are not.
        let f = fields(line).map_err(|e| format!("Line {}: {e}", line_no + 1))?;
        let bad = || format!("Invalid scan record at line {}", line_no + 1);
        if f.len() != 7 { return Err(bad()); }
        let mac: Vec<_> = f[1].split(':').collect();
        if mac.len() != 6 || mac.iter().any(|s| s.len() != 2 || !s.bytes().all(|b| b.is_ascii_hexdigit())) {
            return Err(bad());
        }
        let channel: u16 = f[2].parse().map_err(|_| bad())?;
        let frequency_mhz: u16 = f[3].strip_suffix(" MHz").unwrap_or(&f[3]).parse().map_err(|_| bad())?;
        let signal: u8 = f[4].parse().map_err(|_| bad())?;
        if channel == 0 || channel > 233 || !(2000..=7200).contains(&frequency_mhz) || signal > 100 {
            return Err(bad());
        }
        if !matches!(f[6].trim(), "" | "*") { return Err(bad()); }
        aps.push(AccessPoint {
            ssid: f[0].clone(), bssid: f[1].to_ascii_uppercase(), channel, frequency_mhz, signal,
            security: if f[5].is_empty() || f[5] == "--" { "Open".into() } else { f[5].clone() },
            connected: f[6].trim() == "*",
        });
    }
    Ok(aps)
}

/// Drain both pipes while waiting, so large AP lists cannot deadlock the child.
fn nmcli(args: &[&str]) -> Result<String, String> {
    let mut child = Command::new("nmcli")
        .env("LC_ALL", "C").env("LANG", "C").env("NM_CLI_COLOR", "never")
        .args(["--wait", "15", "--colors", "no"])
        .args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .spawn().map_err(|e| format!("Cannot run nmcli: {e}"))?;
    let mut out = child.stdout.take().unwrap();
    let mut err = child.stderr.take().unwrap();
    let output = thread::spawn(move || { let mut b = Vec::new(); out.read_to_end(&mut b).map(|_| b) });
    let errors = thread::spawn(move || { let mut b = Vec::new(); err.read_to_end(&mut b).map(|_| b) });
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break Ok(s),
            Ok(None) if started.elapsed() < LIMIT => thread::sleep(Duration::from_millis(50)),
            Ok(None) => { let _ = child.kill(); let _ = child.wait(); break Err("nmcli timed out".to_string()); }
            Err(e) => { let _ = child.kill(); let _ = child.wait(); break Err(format!("nmcli wait: {e}")); }
        }
    };
    let stdout = output.join().map_err(|_| "nmcli reader failed")?.map_err(|e| e.to_string())?;
    let stderr = errors.join().map_err(|_| "nmcli error reader failed")?.map_err(|e| e.to_string())?;
    let status = status?;
    if !status.success() {
        return Err(format!("nmcli failed ({}): {}", status.code().unwrap_or(-1), String::from_utf8_lossy(&stderr).trim()));
    }
    String::from_utf8(stdout).map_err(|_| "nmcli returned invalid UTF-8".into())
}

fn ready(interface: &str) -> Result<(), String> {
    if nmcli(&["radio", "wifi"])?.trim() != "enabled" {
        return Err("Wi-Fi radio is disabled".into());
    }
    let raw = nmcli(&["--get-values", "GENERAL.NM-MANAGED,GENERAL.STATE", "device", "show", interface])?;
    let mut lines = raw.lines();
    if lines.next() != Some("yes") { return Err("Wi-Fi interface is not NM-managed".into()); }
    let state: u32 = lines.next().and_then(|s| s.split_whitespace().next()).and_then(|s| s.parse().ok())
        .ok_or("Invalid NetworkManager device state")?;
    if !(30..=100).contains(&state) { return Err(format!("Wi-Fi interface unavailable (state {state})")); }
    Ok(())
}

pub fn scan() -> Result<Scan, String> {
    let raw = nmcli(&["--terse", "--escape", "yes", "--fields", "DEVICE,TYPE,STATE", "device", "status"])?;
    let mut devices = Vec::new();
    for line in raw.lines() {
        let f = fields(line)?;
        if f.len() != 3 { return Err("Invalid nmcli device list".into()); }
        if f[1] == "wifi" { devices.push(f); }
    }
    // Prefer the connected managed radio, otherwise a usable disconnected radio.
    devices.sort_by_key(|f| if f[2] == "connected" { 0 } else { 1 });
    if devices.is_empty() { return Err("No wireless interface detected".into()); }
    let mut reasons = Vec::new();
    for f in devices {
        if let Err(e) = ready(&f[0]) { reasons.push(format!("{}: {e}", f[0])); continue; }
        let raw = nmcli(&["--terse", "--escape", "yes", "--fields", FIELDS,
            "device", "wifi", "list", "ifname", &f[0], "--rescan", "yes"])?;
        ready(&f[0])?; // Empty output from a radio disabled mid-scan is not success.
        let aps = parse(&raw)?;
        return Ok(Scan { interface: f[0].clone(), raw, aps });
    }
    Err(reasons.join("; "))
}

pub enum Event { Started, Finished(Result<Scan, String>) }
enum Request { Refresh, Stop }

pub struct Worker {
    requests: SyncSender<Request>,
    pub events: Receiver<Event>,
}

impl Worker {
    pub fn start() -> Self {
        let (requests, pending) = mpsc::sync_channel(1);
        let (notify, events) = mpsc::channel();
        thread::spawn(move || loop {
            if notify.send(Event::Started).is_err() { break; }
            if notify.send(Event::Finished(scan())).is_err() { break; }
            // Ignore clicks queued during the scan. There is at most one scan worker.
            let mut stop = false;
            while let Ok(request) = pending.try_recv() {
                if matches!(request, Request::Stop) { stop = true; }
            }
            if stop { break; }
            match pending.recv_timeout(REFRESH) {
                Ok(Request::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Ok(Request::Refresh) | Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
        });
        Self { requests, events }
    }

    pub fn refresh(&self) { let _ = self.requests.try_send(Request::Refresh); }
}

impl Drop for Worker {
    fn drop(&mut self) { let _ = self.requests.try_send(Request::Stop); }
}

#[cfg(test)]
mod tests {
    use super::*;
    const ROW: &str = r"Office:AA\:BB\:CC\:DD\:EE\:01:6:2437 MHz:73:WPA2:*";
    #[test] fn bssid_colons_and_connected() {
        let a = parse(ROW).unwrap().remove(0);
        assert_eq!(a.bssid, "AA:BB:CC:DD:EE:01");
        assert_eq!((a.signal, a.channel, a.frequency_mhz, a.connected), (73, 6, 2437, true));
    }
    #[test] fn duplicate_ssids_stay_separate() {
        let raw = format!("{ROW}\n{}", ROW.replace("EE\\:01", "EE\\:02"));
        let a = parse(&raw).unwrap(); assert_eq!(a.len(), 2); assert_ne!(a[0].bssid, a[1].bssid);
    }
    #[test] fn hidden_ssid_and_open_security() {
        let a = parse(&ROW.replace("Office:", ":").replace("WPA2", "--")).unwrap();
        assert!(a[0].ssid.is_empty()); assert_eq!(a[0].security, "Open");
    }
    #[test] fn escaped_ssid_and_backslash() {
        let a = parse(&ROW.replace("Office", r"Lab\:west\\floor")).unwrap();
        assert_eq!(a[0].ssid, r"Lab:west\floor");
    }
    #[test] fn empty_result() { assert!(parse("").unwrap().is_empty()); }
    #[test] fn malformed_output_is_not_silently_zero_aps() {
        for bad in ["Error: NetworkManager is not running", "\n", "SSID:BSSID:CHAN:FREQ:SIGNAL:SECURITY:IN-USE"] {
            assert!(parse(bad).is_err(), "{bad:?}");
        }
        for bad in [ROW.replace(":73:", ":101:"), ROW.replace(":6:", ":x:"), ROW.replace("2437 MHz", "unknown"),
            ROW.replace("AA\\:BB", "AA:BB"), ROW.replace("Office", "bad\\q"), format!("{ROW}\\"),
            ROW.replace("EE\\:01", "EE\\:ZZ"), ROW.replace(":*", ":maybe")] {
            assert!(parse(&bad).is_err(), "{bad:?}");
        }
    }
    #[test] fn literal_dash_ssid_and_percent_endpoints() {
        let raw = format!("{}\r\n{}\r\n", ROW.replace("Office", "--").replace(":73:", ":0:"), ROW.replace(":73:", ":100:"));
        let a = parse(&raw).unwrap(); assert_eq!(a[0].ssid, "--"); assert_eq!((a[0].signal, a[1].signal), (0, 100));
    }
}
