# NFC Bridge Windows

High-performance Windows companion client, synchronization daemon, and diagnostic tools for **NFC Bridge**.

## Overview

`nfc-bridge-windows` provides the PC side for connecting Android smartphones running **NFC Bridge** to Windows, allowing the phone to act as an ultra-low latency (<1 ms) physical or virtual PC/SC NFC smart card reader.

## Architecture

This workspace contains three primary crates:

- **`crates/core-proto`**: Shared Bridge APDU Protocol (BAP) implementation, binary framing, checksum verification, and pairing handshakes.
- **`crates/nfc-bridge-sync`**: Lightweight companion app and System Tray background daemon. Integrates the Google Stitch UI design, manages connections (USB ADB tunnel, Local Network TCP, Bluetooth, Wi-Fi Direct), and automatically checks for and downloads the latest driver from [nfc-bridge-driver](https://github.com/HectorZaGa/nfc-bridge-driver).
- **`crates/nfc-bridge-tools`**: Professional diagnostic and analysis suite for deep APDU inspection, PC/SC sniffing, tag dumping, and emulation.

## Requirements

- **OS**: Windows 10 / Windows 11 (x64, ARM64)
- **Rust**: 1.80+ (Rust 2021 edition)
- **Smart Card Driver**: Automatically checked and downloaded from [`nfc-bridge-driver`](https://github.com/HectorZaGa/nfc-bridge-driver) releases.

## License

GNU General Public License v3.0 (`GPL-3.0-only`). Copyright (c) 2026 Hector Zarate (`@HectorZaGa`). See [LICENSE](LICENSE).

## Code Signing Policy

Free code signing provided by [SignPath.io](https://signpath.io/), certificate by [SignPath Foundation](https://signpath.org/).

- **Committers & Approvers:** Hector Zarate (`@HectorZaGa`)
- **Reviewers:** Repository maintainers (`@HectorZaGa`)
- **Privacy Policy:** Signed binaries are built exclusively from public source code via GitHub Actions. This software does not transmit personal data or telemetry to external servers without explicit user action (such as checking GitHub Releases for updates or using user-configured Relay connections).
