import XCTest
@testable import PulseSDK

final class PulseSDKTests: XCTestCase {
    
    func testIdentityGeneration() throws {
        let identity = PulseIdentity()
        XCTAssertFalse(identity.publicKeyHex.isEmpty)
        XCTAssertFalse(identity.privateKeyHex.isEmpty)
    }
    
    func testHeartbeatSigning() throws {
        let identity = PulseIdentity()
        
        var heartbeat = Heartbeat(
            timestamp: UInt64(Date().timeIntervalSince1970 * 1000),
            heartRate: 72,
            motion: Motion(x: 0.1, y: 0.1, z: 0.05),
            temperature: 36.5,
            devicePubkey: identity.publicKeyHex
        )
        
        try identity.sign(heartbeat: &heartbeat)
        XCTAssertFalse(heartbeat.signature.isEmpty)
    }
    
    func testHeartbeatWeight() {
        let heartbeat = Heartbeat(
            timestamp: 0,
            heartRate: 70,
            motion: Motion(x: 0, y: 0, z: 0),
            temperature: 36.5,
            devicePubkey: ""
        )
        
        // At resting HR (70) with no motion, weight should be ~0.6
        XCTAssertEqual(heartbeat.weight, 0.6, accuracy: 0.1)
    }
    
    func testMotionMagnitude() {
        let motion = Motion(x: 1.0, y: 0, z: 0)
        XCTAssertEqual(motion.magnitude, 1.0, accuracy: 0.001)
        
        let motion2 = Motion(x: 3.0, y: 4.0, z: 0)
        XCTAssertEqual(motion2.magnitude, 5.0, accuracy: 0.001)
    }
}
