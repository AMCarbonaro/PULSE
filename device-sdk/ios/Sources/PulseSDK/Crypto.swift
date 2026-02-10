import Foundation
import Crypto

/// Manages device identity and cryptographic operations
public class PulseIdentity {
    
    private let privateKey: P256.Signing.PrivateKey
    public let publicKey: P256.Signing.PublicKey
    
    /// Public key as hex string (for network identification)
    public var publicKeyHex: String {
        publicKey.rawRepresentation.map { String(format: "%02x", $0) }.joined()
    }
    
    /// Private key as hex string (for backup/restore)
    public var privateKeyHex: String {
        privateKey.rawRepresentation.map { String(format: "%02x", $0) }.joined()
    }
    
    /// Generate a new random identity
    public init() {
        self.privateKey = P256.Signing.PrivateKey()
        self.publicKey = privateKey.publicKey
    }
    
    /// Restore identity from private key hex
    public init(privateKeyHex: String) throws {
        let keyData = Data(hexString: privateKeyHex)
        self.privateKey = try P256.Signing.PrivateKey(rawRepresentation: keyData)
        self.publicKey = privateKey.publicKey
    }
    
    /// Sign data and return hex-encoded signature
    public func sign(_ data: Data) throws -> String {
        let signature = try privateKey.signature(for: data)
        return signature.rawRepresentation.map { String(format: "%02x", $0) }.joined()
    }
    
    /// Sign a heartbeat packet
    public func sign(heartbeat: inout Heartbeat) throws {
        let signableData = try heartbeat.signableData()
        heartbeat.signature = try sign(signableData)
    }
    
    // MARK: - Keychain Storage
    
    private static let keychainService = "com.pulse.identity"
    private static let keychainAccount = "device_private_key"
    
    /// Save identity to Keychain
    public func saveToKeychain() throws {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: Self.keychainService,
            kSecAttrAccount as String: Self.keychainAccount,
            kSecValueData as String: privateKey.rawRepresentation
        ]
        
        // Delete existing
        SecItemDelete(query as CFDictionary)
        
        // Add new
        let status = SecItemAdd(query as CFDictionary, nil)
        guard status == errSecSuccess else {
            throw PulseError.keychainError(status)
        }
    }
    
    /// Load identity from Keychain
    public static func loadFromKeychain() throws -> PulseIdentity? {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: keychainService,
            kSecAttrAccount as String: keychainAccount,
            kSecReturnData as String: true
        ]
        
        var result: AnyObject?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        
        guard status == errSecSuccess, let keyData = result as? Data else {
            if status == errSecItemNotFound {
                return nil
            }
            throw PulseError.keychainError(status)
        }
        
        let privateKey = try P256.Signing.PrivateKey(rawRepresentation: keyData)
        return try PulseIdentity(privateKeyHex: keyData.map { String(format: "%02x", $0) }.joined())
    }
    
    /// Get or create identity (loads from Keychain or generates new)
    public static func getOrCreate() throws -> PulseIdentity {
        if let existing = try loadFromKeychain() {
            return existing
        }
        
        let identity = PulseIdentity()
        try identity.saveToKeychain()
        return identity
    }
}

// MARK: - Errors

public enum PulseError: Error {
    case keychainError(OSStatus)
    case networkError(String)
    case invalidResponse
    case healthKitNotAvailable
    case healthKitAuthorizationDenied
}

// MARK: - Data Extension

extension Data {
    init(hexString: String) {
        self.init()
        var hex = hexString
        while hex.count >= 2 {
            let byteString = String(hex.prefix(2))
            hex = String(hex.dropFirst(2))
            if let byte = UInt8(byteString, radix: 16) {
                self.append(byte)
            }
        }
    }
}
