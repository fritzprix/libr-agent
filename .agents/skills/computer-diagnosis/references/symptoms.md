# Symptom Database

A quick reference guide mapping common user symptoms to potential system metrics.

## 1. Slowness / Freezing

- **Keywords**: "slow", "lag", "freezing", "unresponsive", "hanging"
- **Primary metrics to check**:
  - High CPU Usage (Check `CPU Info -> Total CPU Usage`)
  - High Memory Usage (Check `Memory Info -> Percentage Virtual Memory`)
  - Disk Space (Check `Disk Info -> Percentage` on system drive)
- **Common causes**: Too many background apps, insufficient RAM, almost full system drive.

## 2. Loud Fan Noise

- **Keywords**: "noisy", "loud fan", "overheating", "hot"
- **Primary metrics to check**:
  - CPU Usage (Check `CPU Info -> Total CPU Usage`)
- **Common causes**: CPU intensive tasks causing thermal throttling.

## 3. Storage Issues

- **Keywords**: "no space", "full disk", "can't download", "can't save"
- **Primary metrics to check**:
  - Disk Space (Check `Disk Info` for all drives, look for `Percentage` > 90%)
- **Common causes**: Accumulated temp files, large media files, forgotten downloads.

## 4. Internet/Network Issues

- **Keywords**: "no internet", "disconnecting", "slow connection", "offline"
- **Primary metrics to check**:
  - Network Interfaces (Check `Network Info -> Interfaces` for assigned IP addresses)
  - Data Transfer (Check `Network Info -> IO` to see if bytes are being sent/received)
- **Common causes**: Adapter issues, missing IP configuration, router problems.
