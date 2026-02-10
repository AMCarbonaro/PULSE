import Foundation
import Combine

/// Main client for the Pulse Network - manages identity, biometrics, and pulsing
public class PulseClient: ObservableObject {
    
    // MARK: - Published State
    
    @Published public private(set) var isConnected = false
    @Published public private(set) var isPulsing = false
    @Published public private(set) var currentHeartRate: Double = 0
    @Published public private(set) var currentMotion: Motion = Motion(x: 0, y: 0, z: 0)
    @Published public private(set) var balance: Double = 0
    @Published public private(set) var lastError: String?
    
    // MARK: - Components
    
    public let identity: PulseIdentity
    private let biometrics: BiometricProvider
    private let network: PulseNetworkClient
    
    private var pulseTimer: Timer?
    private var balanceTimer: Timer?
    
    /// Interval between heartbeat submissions (seconds)
    public var pulseInterval: TimeInterval = 5.0
    
    // MARK: - Initialization
    
    /// Create a new PulseClient
    /// - Parameters:
    ///   - nodeURL: URL of the Pulse node (e.g., "http://localhost:8080")
    ///   - identity: Optional existing identity, or nil to create/load from Keychain
    public init(nodeURL: String, identity: PulseIdentity? = nil) throws {
        self.identity = try identity ?? PulseIdentity.getOrCreate()
        self.biometrics = BiometricProvider()
        self.network = PulseNetworkClient(nodeURL: nodeURL)
    }
    
    // MARK: - Public API
    
    /// Your public key (network identifier)
    public var publicKey: String {
        identity.publicKeyHex
    }
    
    /// Request HealthKit authorization
    public func requestAuthorization() async throws {
        try await biometrics.requestAuthorization()
    }
    
    /// Connect to the node and verify connectivity
    public func connect() async throws {
        let healthy = try await network.healthCheck()
        await MainActor.run {
            self.isConnected = healthy
        }
        
        if healthy {
            // Start balance updates
            startBalanceUpdates()
        }
    }
    
    /// Start pulsing (capturing and submitting heartbeats)
    public func startPulsing() {
        guard !isPulsing else { return }
        
        // Start biometric capture
        biometrics.startHeartRateUpdates { [weak self] hr in
            DispatchQueue.main.async {
                self?.currentHeartRate = hr
            }
        }
        
        biometrics.startMotionUpdates { [weak self] motion in
            DispatchQueue.main.async {
                self?.currentMotion = motion
            }
        }
        
        // Start pulse timer
        pulseTimer = Timer.scheduledTimer(withTimeInterval: pulseInterval, repeats: true) { [weak self] _ in
            Task {
                await self?.pulse()
            }
        }
        
        // Pulse immediately
        Task {
            await pulse()
        }
        
        isPulsing = true
    }
    
    /// Stop pulsing
    public func stopPulsing() {
        pulseTimer?.invalidate()
        pulseTimer = nil
        biometrics.stopMotionUpdates()
        isPulsing = false
    }
    
    /// Get current network stats
    public func getNetworkStats() async throws -> NetworkStats {
        try await network.getStats()
    }
    
    /// Refresh balance
    public func refreshBalance() async {
        do {
            let newBalance = try await network.getBalance(pubkey: publicKey)
            await MainActor.run {
                self.balance = newBalance
            }
        } catch {
            // Silently ignore balance fetch errors
        }
    }
    
    // MARK: - Private
    
    private func pulse() async {
        do {
            let heartbeat = try biometrics.captureHeartbeat(identity: identity)
            try await network.submitHeartbeat(heartbeat)
            
            await MainActor.run {
                self.lastError = nil
            }
        } catch {
            await MainActor.run {
                self.lastError = error.localizedDescription
            }
        }
    }
    
    private func startBalanceUpdates() {
        balanceTimer?.invalidate()
        balanceTimer = Timer.scheduledTimer(withTimeInterval: 10, repeats: true) { [weak self] _ in
            Task {
                await self?.refreshBalance()
            }
        }
        
        // Fetch immediately
        Task {
            await refreshBalance()
        }
    }
    
    deinit {
        stopPulsing()
        balanceTimer?.invalidate()
    }
}

// MARK: - SwiftUI Convenience

#if canImport(SwiftUI) && (os(iOS) || os(watchOS))
import SwiftUI

public extension PulseClient {
    /// Formatted balance string
    var formattedBalance: String {
        String(format: "%.4f PULSE", balance)
    }
    
    /// Status text
    var statusText: String {
        if !isConnected {
            return "Disconnected"
        } else if isPulsing {
            return "Pulsing..."
        } else {
            return "Connected"
        }
    }
    
    /// Status color
    var statusColor: Color {
        if !isConnected {
            return .red
        } else if isPulsing {
            return .green
        } else {
            return .yellow
        }
    }
}
#endif
