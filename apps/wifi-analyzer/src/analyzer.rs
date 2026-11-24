use std::collections::BTreeMap;
use crate::scanner::AccessPoint;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Band { Ghz24, Ghz5, Other }
impl Band {
    pub fn of(ap: &AccessPoint) -> Self {
        match ap.frequency_mhz { 2400..=2500 => Self::Ghz24, 4900..=5895 => Self::Ghz5, _ => Self::Other }
    }
    pub fn label(self) -> &'static str { match self { Self::Ghz24 => "2.4GHz", Self::Ghz5 => "5GHz", Self::Other => "Other" } }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub ghz24: usize,
    pub ghz5: usize,
    pub other: usize,
    pub channels24: BTreeMap<u16, usize>,
    pub channels5: BTreeMap<u16, usize>,
}

/// BSSID is the identity; strongest duplicate wins and ties sort by BSSID.
pub fn analyze(aps: Vec<AccessPoint>) -> (Vec<AccessPoint>, Summary) {
    let mut unique: BTreeMap<String, AccessPoint> = BTreeMap::new();
    for ap in aps {
        unique.entry(ap.bssid.clone()).and_modify(|old| {
            let connected = old.connected || ap.connected;
            if ap.signal > old.signal { *old = ap.clone(); }
            old.connected = connected;
        }).or_insert(ap);
    }
    let mut aps: Vec<_> = unique.into_values().collect();
    aps.sort_by(|a,b| b.signal.cmp(&a.signal).then_with(|| a.bssid.cmp(&b.bssid)));
    let mut summary = Summary::default();
    for ap in &aps {
        match Band::of(ap) {
            Band::Ghz24 => { summary.ghz24 += 1; *summary.channels24.entry(ap.channel).or_default() += 1; }
            Band::Ghz5 => { summary.ghz5 += 1; *summary.channels5.entry(ap.channel).or_default() += 1; }
            Band::Other => summary.other += 1,
        }
    }
    (aps, summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::parse;
    #[test] fn dedup_sort_and_channel_counts() {
        let raw = concat!(
            "Same:AA\\:BB\\:CC\\:DD\\:EE\\:01:1:2412 MHz:20:WPA2:*\n",
            "Same:AA\\:BB\\:CC\\:DD\\:EE\\:02:36:5180 MHz:80:WPA2: \n",
            "Same:AA\\:BB\\:CC\\:DD\\:EE\\:01:1:2412 MHz:40:WPA2: \n",
            ":AA\\:BB\\:CC\\:DD\\:EE\\:03:1:2412 MHz:40:WPA2: \n");
        let (aps,s) = analyze(parse(raw).unwrap());
        assert_eq!(aps.len(), 3); assert_eq!(aps[0].signal, 80); assert!(aps[1].connected);
        assert_eq!((s.ghz24,s.ghz5,s.channels24[&1],s.channels5[&36]), (2,1,2,1));
        assert!(aps[1].bssid < aps[2].bssid);
    }
    #[test] fn empty_summary() { assert_eq!(analyze(vec![]), (vec![], Summary::default())); }
    #[test] fn six_ghz_is_not_misreported_as_24ghz() {
        let (a,s) = analyze(parse("x:AA\\:BB\\:CC\\:DD\\:EE\\:01:1:5955 MHz:10:--: ").unwrap());
        assert_eq!(Band::of(&a[0]), Band::Other); assert_eq!((s.ghz24,s.ghz5,s.other), (0,0,1));
    }
}
