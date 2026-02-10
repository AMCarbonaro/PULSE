import Foundation
import HealthKit
import CoreMotion

/// Captures biometric data from HealthKit and CoreMotion
public class BiometricProvider {
    
    private let healthStore = HKHealthStore()
    private let motionManager = CMMotionManager()
    
    private var latestHeartRate: Double = 0
    private var latestMotion: Motion = Motion(x: 0, y: 0, z: 0)
    
    public init() {}
    
    // MARK: - Authorization
    
    /// Check if HealthKit is available on this device
    public var isHealthKitAvailable: Bool {
        HKHealthStore.isHealthDataAvailable()
    }
    
    /// Request authorization to read health data
    public func requestAuthorization() async throws {
        guard isHealthKitAvailable else {
            throw PulseError.healthKitNotAvailable
        }
        
        let typesToRead: Set<HKSampleType> = [
            HKQuantityType(.heartRate),
            HKQuantityType(.bodyTemperature),
            HKQuantityType(.oxygenSaturation)
        ]
        
        try await healthStore.requestAuthorization(toShare: [], read: typesToRead)
    }
    
    // MARK: - Heart Rate
    
    /// Start observing heart rate updates
    public func startHeartRateUpdates(handler: @escaping (Double) -> Void) {
        let heartRateType = HKQuantityType(.heartRate)
        
        // Query for most recent heart rate
        let sortDescriptor = NSSortDescriptor(
            key: HKSampleSortIdentifierEndDate,
            ascending: false
        )
        
        let query = HKSampleQuery(
            sampleType: heartRateType,
            predicate: nil,
            limit: 1,
            sortDescriptors: [sortDescriptor]
        ) { [weak self] _, samples, _ in
            guard let sample = samples?.first as? HKQuantitySample else { return }
            let heartRate = sample.quantity.doubleValue(for: HKUnit.count().unitDivided(by: .minute()))
            self?.latestHeartRate = heartRate
            handler(heartRate)
        }
        
        healthStore.execute(query)
        
        // Set up observer for real-time updates
        let observerQuery = HKObserverQuery(
            sampleType: heartRateType,
            predicate: nil
        ) { [weak self] _, completionHandler, _ in
            self?.fetchLatestHeartRate(handler: handler)
            completionHandler()
        }
        
        healthStore.execute(observerQuery)
    }
    
    private func fetchLatestHeartRate(handler: @escaping (Double) -> Void) {
        let heartRateType = HKQuantityType(.heartRate)
        let sortDescriptor = NSSortDescriptor(
            key: HKSampleSortIdentifierEndDate,
            ascending: false
        )
        
        let query = HKSampleQuery(
            sampleType: heartRateType,
            predicate: nil,
            limit: 1,
            sortDescriptors: [sortDescriptor]
        ) { [weak self] _, samples, _ in
            guard let sample = samples?.first as? HKQuantitySample else { return }
            let heartRate = sample.quantity.doubleValue(for: HKUnit.count().unitDivided(by: .minute()))
            self?.latestHeartRate = heartRate
            DispatchQueue.main.async {
                handler(heartRate)
            }
        }
        
        healthStore.execute(query)
    }
    
    // MARK: - Motion
    
    /// Start accelerometer updates
    public func startMotionUpdates(handler: @escaping (Motion) -> Void) {
        guard motionManager.isAccelerometerAvailable else { return }
        
        motionManager.accelerometerUpdateInterval = 0.5
        motionManager.startAccelerometerUpdates(to: .main) { [weak self] data, _ in
            guard let acceleration = data?.acceleration else { return }
            let motion = Motion(
                x: acceleration.x,
                y: acceleration.y,
                z: acceleration.z
            )
            self?.latestMotion = motion
            handler(motion)
        }
    }
    
    /// Stop motion updates
    public func stopMotionUpdates() {
        motionManager.stopAccelerometerUpdates()
    }
    
    // MARK: - Heartbeat Capture
    
    /// Capture current biometrics as a Heartbeat packet
    public func captureHeartbeat(identity: PulseIdentity) throws -> Heartbeat {
        var heartbeat = Heartbeat(
            timestamp: UInt64(Date().timeIntervalSince1970 * 1000),
            heartRate: UInt16(latestHeartRate),
            motion: latestMotion,
            temperature: 36.5, // TODO: Get from HealthKit if available
            devicePubkey: identity.publicKeyHex
        )
        
        try identity.sign(heartbeat: &heartbeat)
        return heartbeat
    }
}

// MARK: - watchOS Workout Session Support

#if os(watchOS)
import WatchKit

extension BiometricProvider {
    
    /// Start a workout session for continuous heart rate monitoring
    public func startWorkoutSession() async throws {
        let configuration = HKWorkoutConfiguration()
        configuration.activityType = .other
        configuration.locationType = .unknown
        
        let session = try HKWorkoutSession(healthStore: healthStore, configuration: configuration)
        let builder = session.associatedWorkoutBuilder()
        
        builder.dataSource = HKLiveWorkoutDataSource(
            healthStore: healthStore,
            workoutConfiguration: configuration
        )
        
        session.startActivity(with: Date())
        try await builder.beginCollection(at: Date())
    }
}
#endif
