import Foundation
import secp256k1

/// Manages device identity and cryptographic operations using secp256k1
/// (compatible with the Pulse node's k256/ECDSA signatures)
public class PulseIdentity {
    
    private let privateKey: secp256k1.Signing.PrivateKey
    public let publicKey: secp256k1.Signing.PublicKey
    
    /// Compressed public key as hex string (33 bytes, for network identification)
    public var publicKeyHex: String {
        publicKey.dataRepresentation.map { String(format: "%02x", $0) }.joined()
    }
    
    /// Private key as hex string (for backup/restore)
    public var privateKeyHex: String {
        privateKey.dataRepresentation.map { String(format: "%02x", $0) }.joined()
    }
    
    /// Generate a new random identity
    public init() {
        self.privateKey = try! secp256k1.Signing.PrivateKey()
        self.publicKey = privateKey.publicKey
    }
    
    /// Restore identity from private key hex (raw 32-byte scalar)
    public init(privateKeyHex: String) throws {
        let keyData = Data(hexString: privateKeyHex)
        self.privateKey = try secp256k1.Signing.PrivateKey(dataRepresentation: keyData)
        self.publicKey = privateKey.publicKey
    }
    
    /// Restore identity from raw private key data
    private init(rawKeyData: Data) throws {
        self.privateKey = try secp256k1.Signing.PrivateKey(dataRepresentation: rawKeyData)
        self.publicKey = privateKey.publicKey
    }
    
    /// Sign data with ECDSA-SHA256 and return hex-encoded compact signature (64 bytes: r‖s)
    /// secp256k1.swift's signature(for:) hashes the message with SHA-256 internally.
    public func sign(_ data: Data) throws -> String {
        let signature = try privateKey.signature(for: data)
        return signature.dataRepresentation.map { String(format: "%02x", $0) }.joined()
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
            kSecValueData as String: privateKey.dataRepresentation
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
        
        return try PulseIdentity(rawKeyData: keyData)
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
