# PulseSDK for iOS/watchOS

Swift SDK for connecting to the Pulse Network - capture heartbeats and earn PULSE tokens.

## Requirements

- iOS 15.0+ / watchOS 8.0+
- Xcode 15.0+
- Swift 5.9+

## Installation

### Swift Package Manager

Add to your `Package.swift`:

```swift
dependencies: [
    .package(path: "../device-sdk/ios")
]
```

Or in Xcode: File → Add Package Dependencies → Add Local...

## Quick Start

### 1. Setup Capabilities

In your Xcode project, enable:
- **HealthKit** (for heart rate)
- **Background Modes** → Background fetch (for continuous pulsing)

Add to `Info.plist`:
```xml
<key>NSHealthShareUsageDescription</key>
<string>Pulse needs heart rate data to verify you're alive and earn tokens.</string>
<key>NSMotionUsageDescription</key>
<string>Pulse uses motion to calculate your activity level.</string>
```

### 2. Initialize the Client

```swift
import PulseSDK

// Create client (generates or loads identity from Keychain)
let client = try PulseClient(nodeURL: "http://your-node:8080")

// Request HealthKit permissions
try await client.requestAuthorization()

// Connect to node
try await client.connect()
```

### 3. Start Pulsing

```swift
// Start capturing and submitting heartbeats
client.startPulsing()

// Check your balance
print("Balance: \(client.balance) PULSE")

// Stop when done
client.stopPulsing()
```

### 4. Use the SwiftUI View

```swift
import SwiftUI
import PulseSDK

struct ContentView: View {
    @StateObject var client = try! PulseClient(nodeURL: "http://localhost:8080")
    
    var body: some View {
        PulseView(client: client)
    }
}
```

## watchOS

For Apple Watch, the SDK includes workout session support for continuous heart rate monitoring:

```swift
#if os(watchOS)
let provider = BiometricProvider()
try await provider.startWorkoutSession()
#endif
```

## API Reference

### PulseClient

| Property | Type | Description |
|----------|------|-------------|
| `isConnected` | `Bool` | Connection status |
| `isPulsing` | `Bool` | Whether actively pulsing |
| `currentHeartRate` | `Double` | Latest heart rate (BPM) |
| `balance` | `Double` | Current PULSE balance |
| `publicKey` | `String` | Your network identity |

| Method | Description |
|--------|-------------|
| `requestAuthorization()` | Request HealthKit permissions |
| `connect()` | Connect to node |
| `startPulsing()` | Begin heartbeat capture/submission |
| `stopPulsing()` | Stop pulsing |
| `getNetworkStats()` | Get network statistics |

### PulseIdentity

```swift
// Generate new identity
let identity = PulseIdentity()

// Get public key for display
print(identity.publicKeyHex)

// Save to Keychain
try identity.saveToKeychain()

// Load from Keychain
let restored = try PulseIdentity.loadFromKeychain()
```

## Architecture

```
PulseSDK/
├── Models.swift           # Heartbeat, Motion, NetworkStats
├── Crypto.swift           # PulseIdentity, signing, Keychain
├── HealthKitProvider.swift # Biometric capture
├── NetworkClient.swift    # HTTP API client
├── PulseClient.swift      # Main client (ObservableObject)
└── PulseView.swift        # Ready-to-use SwiftUI view
```

## Security

- Private keys are stored in the iOS Keychain (Secure Enclave when available)
- All heartbeat packets are signed with ECDSA P-256
- Network communication should use HTTPS in production

## License

MIT
