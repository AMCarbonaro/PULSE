import Foundation

/// Motion vector from accelerometer
public struct Motion: Codable {
    public let x: Double
    public let y: Double
    public let z: Double
    
    public init(x: Double, y: Double, z: Double) {
        self.x = x
        self.y = y
        self.z = z
    }
    
    public var magnitude: Double {
        sqrt(x * x + y * y + z * z)
    }
}

/// A heartbeat packet - the atomic unit of Proof-of-Life
public struct Heartbeat: Codable {
    /// Unix timestamp in milliseconds
    public let timestamp: UInt64
    /// Heart rate in BPM
    public let heartRate: UInt16
    /// Motion vector from accelerometer
    public let motion: Motion
    /// Body temperature in Celsius
    public let temperature: Float
    /// Device/user public key (hex-encoded)
    public let devicePubkey: String
    /// ECDSA signature of the packet (hex-encoded)
    public var signature: String
    
    enum CodingKeys: String, CodingKey {
        case timestamp
        case heartRate = "heart_rate"
        case motion
        case temperature
        case devicePubkey = "device_pubkey"
        case signature
    }
    
    public init(
        timestamp: UInt64,
        heartRate: UInt16,
        motion: Motion,
        temperature: Float,
        devicePubkey: String,
        signature: String = ""
    ) {
        self.timestamp = timestamp
        self.heartRate = heartRate
        self.motion = motion
        self.temperature = temperature
        self.devicePubkey = devicePubkey
        self.signature = signature
    }
    
    /// Calculate weighted contribution
    public var weight: Double {
        let alpha = 0.4  // Heart rate weight
        let beta = 0.4   // Motion weight
        let gamma = 0.2  // Continuity weight
        
        let hrNorm = Double(heartRate) / 70.0
        let motionNorm = min(motion.magnitude / 0.5, 2.0)
        let continuity = 1.0
        
        return alpha * hrNorm + beta * motionNorm + gamma * continuity
    }
    
    /// Get signable data (excludes signature)
    public func signableData() throws -> Data {
        let signable: [String: Any] = [
            "timestamp": timestamp,
            "heart_rate": heartRate,
            "motion": ["x": motion.x, "y": motion.y, "z": motion.z],
            "temperature": temperature,
            "device_pubkey": devicePubkey
        ]
        return try JSONSerialization.data(withJSONObject: signable, options: .sortedKeys)
    }
}

/// API response wrapper
public struct APIResponse<T: Decodable>: Decodable {
    public let success: Bool
    public let data: T?
    public let error: String?
}

/// Network statistics
public struct NetworkStats: Codable {
    public let chainLength: UInt64
    public let totalMinted: Double
    public let activeAccounts: Int
    public let currentTps: Double
    public let avgBlockTime: Double
    public let totalSecurity: Double
    
    enum CodingKeys: String, CodingKey {
        case chainLength = "chain_length"
        case totalMinted = "total_minted"
        case activeAccounts = "active_accounts"
        case currentTps = "current_tps"
        case avgBlockTime = "avg_block_time"
        case totalSecurity = "total_security"
    }
}
