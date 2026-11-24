#!/usr/bin/env python3
"""Independently compare unmodified nmcli terse output with the UI snapshot."""
import csv
import pathlib
import sys

directory = pathlib.Path(sys.argv[1])
records = {}
for row in csv.reader((directory / 'nmcli.txt').read_text().splitlines(), delimiter=':', escapechar='\\', quoting=csv.QUOTE_NONE):
    assert len(row) == 7, row
    ssid, bssid, channel, frequency, signal, security, active = row
    bssid = bssid.upper()
    value = (int(signal), int(channel), int(frequency.removesuffix(' MHz')), active.strip() == '*')
    old = records.get(bssid)
    if old:
        strongest = max((old, value), key=lambda x: x[0])
        value = (*strongest[:3], old[3] or value[3])
    records[bssid] = value

lines = (directory / 'ui.tsv').read_text().splitlines()
metadata = dict(line.split('\t') for line in lines[:6])
ui = {}
order = []
for line in lines[7:]:
    bssid, signal, channel, frequency, connected = line.split('\t')
    ui[bssid] = (int(signal), int(channel), int(frequency), connected == 'true')
    order.append(bssid)
assert ui == records, (ui, records)
assert int(metadata['APs']) == len(records)
assert int(metadata['2.4GHz']) == sum(2400 <= ap[2] <= 2500 for ap in records.values())
assert int(metadata['5GHz']) == sum(4900 <= ap[2] <= 5895 for ap in records.values())
assert int(metadata['Other']) == sum(not (2400 <= ap[2] <= 2500 or 4900 <= ap[2] <= 5895) for ap in records.values())
assert order == sorted(records, key=lambda b: (-records[b][0], b))
expected_channels = {}
for signal, channel, frequency, _ in records.values():
    band = '2.4GHz' if 2400 <= frequency <= 2500 else '5GHz' if 4900 <= frequency <= 5895 else None
    if band:
        count, maximum = expected_channels.get((band, channel), (0, 0))
        expected_channels[(band, channel)] = (count + 1, max(maximum, signal))
actual_channels = {}
for line in (directory / 'channels.tsv').read_text().splitlines()[1:]:
    band, channel, count, signal = line.split('\t')
    actual_channels[(band, int(channel))] = (int(count), int(signal))
assert actual_channels == expected_channels, (actual_channels, expected_channels)
print(f"PASS: {len(records)} APs; all BSSIDs, signal percentages, channels, MHz, connection flags, band totals, channel charts and sort order match raw nmcli")
for bssid in order[:4]:
    print(bssid, records[bssid])
