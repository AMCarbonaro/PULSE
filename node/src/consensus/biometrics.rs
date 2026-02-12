//! Biometric validation and entropy extraction for Proof-of-Life.
//!
//! This module provides:
//! 1. Heart Rate Variability (HRV) analysis to detect synthetic heartbeats
//! 2. Biometric entropy extraction for block randomness
//! 3. Anomaly detection for spoofed sensor data

use std::collections::VecDeque;
use tracing::{warn, debug};

/// Maximum history per device for HRV analysis
const MAX_HR_HISTORY: usize = 60; // ~5 minutes at 5s intervals
const MAX_MOTION_HISTORY: usize = 60;

/// Biometric validator that tracks per-device history for anomaly detection
pub struct BiometricValidator {
    /// Heart rate history per device (pubkey -> recent HR values)
    hr_history: std::collections::HashMap<String, VecDeque<u16>>,
    /// Motion history per device
    motion_history: std::collections::HashMap<String, VecDeque<f64>>,
}

/// Result of biometric validation
#[derive(Debug, Clone)]
pub struct BiometricResult {
    /// Is this heartbeat likely from a real human?
    pub is_valid: bool,
    /// Confidence score [0, 1] — how confident we are this is real
    pub confidence: f64,
    /// Reason for rejection (if invalid)
    pub reason: Option<String>,
    /// Extracted entropy from biometric variability (bytes)
    pub entropy_bits: Vec<u8>,
    /// Heart rate variability (SDNN in BPM) — 0 if not enough history
    pub hrv_sdnn: f64,
}

impl BiometricValidator {
    pub fn new() -> Self {
        Self {
            hr_history: std::collections::HashMap::new(),
            motion_history: std::collections::HashMap::new(),
        }
    }

    /// Validate a heartbeat and extract biometric entropy.
    /// Returns validation result with confidence score and entropy.
    pub fn validate(
        &mut self,
        device_pubkey: &str,
        heart_rate: u16,
        motion_magnitude: f64,
        temperature: f32,
    ) -> BiometricResult {
        let mut confidence = 1.0;
        let mut reasons: Vec<String> = Vec::new();

        // --- 1. Physiological range checks ---
        
        // Temperature should be in human range
        if temperature < 33.0 || temperature > 42.0 {
            confidence *= 0.3;
            reasons.push(format!("Temperature {:.1}°C outside human range", temperature));
        }
        
        // Heart rate physiological bounds (already checked in consensus, but double-check)
        if heart_rate < 30 || heart_rate > 220 {
            return BiometricResult {
                is_valid: false,
                confidence: 0.0,
                reason: Some(format!("HR {} outside physiological bounds", heart_rate)),
                entropy_bits: vec![],
                hrv_sdnn: 0.0,
            };
        }

        // --- 2. Heart Rate Variability (HRV) analysis ---
        // Real human hearts have natural variability (SDNN typically 20-200ms).
        // Constant or perfectly periodic HR = synthetic/spoofed signal.
        
        let hr_queue = self.hr_history
            .entry(device_pubkey.to_string())
            .or_insert_with(|| VecDeque::with_capacity(MAX_HR_HISTORY));
        
        hr_queue.push_back(heart_rate);
        if hr_queue.len() > MAX_HR_HISTORY {
            hr_queue.pop_front();
        }
        
        let hrv_sdnn = if hr_queue.len() >= 5 {
            Self::calculate_sdnn(hr_queue)
        } else {
            0.0 // Not enough data yet
        };
        
        if hr_queue.len() >= 10 {
            // Check for suspiciously LOW variability (constant HR = likely fake)
            if hrv_sdnn < 0.5 {
                confidence *= 0.2;
                reasons.push(format!("HRV too low ({:.2} BPM SDNN) — possible synthetic signal", hrv_sdnn));
            }
            
            // Check for suspiciously HIGH variability (random noise = likely fake)
            if hrv_sdnn > 40.0 {
                confidence *= 0.4;
                reasons.push(format!("HRV too high ({:.2} BPM SDNN) — possible random noise", hrv_sdnn));
            }
            
            // Check for perfectly periodic patterns (e.g., HR alternating between 2 values)
            if Self::is_periodic(hr_queue) {
                confidence *= 0.3;
                reasons.push("HR shows periodic pattern — possible synthetic oscillator".to_string());
            }
        }

        // --- 3. Motion plausibility ---
        // Real humans have correlated HR and motion — resting HR should come
        // with low motion, high HR with higher motion (usually)
        
        let motion_queue = self.motion_history
            .entry(device_pubkey.to_string())
            .or_insert_with(|| VecDeque::with_capacity(MAX_MOTION_HISTORY));
        
        motion_queue.push_back(motion_magnitude);
        if motion_queue.len() > MAX_MOTION_HISTORY {
            motion_queue.pop_front();
        }
        
        // Gross mismatch: very high HR but zero motion for extended period
        if hr_queue.len() >= 10 && motion_queue.len() >= 10 {
            let avg_hr: f64 = hr_queue.iter().map(|h| *h as f64).sum::<f64>() / hr_queue.len() as f64;
            let avg_motion: f64 = motion_queue.iter().sum::<f64>() / motion_queue.len() as f64;
            
            // High HR (>130) with essentially no motion for 10+ readings
            if avg_hr > 130.0 && avg_motion < 0.05 {
                confidence *= 0.5;
                reasons.push(format!(
                    "HR/motion mismatch: avg HR={:.0} but avg motion={:.3}", avg_hr, avg_motion
                ));
            }
            
            // Constant motion magnitude (real accelerometers have noise)
            let motion_sdnn = Self::calculate_sdnn_f64(motion_queue);
            if motion_queue.len() >= 10 && motion_sdnn < 0.001 && avg_motion > 0.01 {
                confidence *= 0.4;
                reasons.push(format!(
                    "Motion too constant (SD={:.6}) — possible synthetic", motion_sdnn
                ));
            }
        }

        // --- 4. Extract biometric entropy ---
        // Use the least significant bits of HR and motion as entropy source.
        // Real biometric data has natural noise = good entropy.
        let entropy_bits = Self::extract_entropy(heart_rate, motion_magnitude, hrv_sdnn);

        // --- Result ---
        let is_valid = confidence >= 0.3; // Threshold: reject only very suspicious
        
        if !is_valid {
            warn!("🚨 Biometric validation FAILED for {}...: confidence={:.2}, reasons: {:?}",
                &device_pubkey[..8.min(device_pubkey.len())], confidence, reasons);
        } else if confidence < 0.7 {
            debug!("⚠️ Biometric confidence low for {}...: {:.2} — {:?}",
                &device_pubkey[..8.min(device_pubkey.len())], confidence, reasons);
        }

        BiometricResult {
            is_valid,
            confidence,
            reason: if reasons.is_empty() { None } else { Some(reasons.join("; ")) },
            entropy_bits,
            hrv_sdnn,
        }
    }

    /// Calculate Standard Deviation of Normal-to-Normal intervals (SDNN)
    /// for heart rate values. This is the primary HRV metric.
    /// Higher SDNN = more variability = healthier/more realistic signal.
    fn calculate_sdnn(values: &VecDeque<u16>) -> f64 {
        if values.len() < 2 { return 0.0; }
        let n = values.len() as f64;
        let mean = values.iter().map(|v| *v as f64).sum::<f64>() / n;
        let variance = values.iter()
            .map(|v| (*v as f64 - mean).powi(2))
            .sum::<f64>() / (n - 1.0);
        variance.sqrt()
    }

    fn calculate_sdnn_f64(values: &VecDeque<f64>) -> f64 {
        if values.len() < 2 { return 0.0; }
        let n = values.len() as f64;
        let mean = values.iter().sum::<f64>() / n;
        let variance = values.iter()
            .map(|v| (v - mean).powi(2))
            .sum::<f64>() / (n - 1.0);
        variance.sqrt()
    }

    /// Detect periodic patterns in HR (e.g., 72, 73, 72, 73, 72, 73...)
    /// Real hearts don't oscillate with perfect periodicity.
    fn is_periodic(values: &VecDeque<u16>) -> bool {
        if values.len() < 8 { return false; }
        
        // Check for period-2 pattern (alternating)
        let recent: Vec<u16> = values.iter().rev().take(8).cloned().collect();
        let mut period2_match = 0;
        for i in 0..6 {
            if recent[i] == recent[i + 2] { period2_match += 1; }
        }
        if period2_match >= 5 { return true; }
        
        // Check for constant value (period-1)
        let unique: std::collections::HashSet<u16> = values.iter().rev().take(10).cloned().collect();
        if unique.len() <= 1 { return true; }
        
        false
    }

    /// Extract entropy from biometric signals.
    /// Uses LSBs of heart rate, motion magnitude, and HRV.
    /// Real biometric data has natural measurement noise → good entropy.
    /// For cryptographic use, this should be fed into a CSPRNG, not used directly.
    fn extract_entropy(heart_rate: u16, motion_mag: f64, hrv: f64) -> Vec<u8> {
        use sha2::{Sha256, Digest};
        
        // Mix biometric values into entropy pool
        let mut hasher = Sha256::new();
        hasher.update(heart_rate.to_le_bytes());
        hasher.update(motion_mag.to_le_bytes());
        hasher.update(hrv.to_le_bytes());
        
        // Add nanosecond timestamp for additional entropy
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        hasher.update(nanos.to_le_bytes());
        
        // Output 32 bytes of entropy
        hasher.finalize().to_vec()
    }

    /// Clean up stale device histories
    pub fn cleanup(&mut self, active_pubkeys: &[String]) {
        let active_set: std::collections::HashSet<&String> = active_pubkeys.iter().collect();
        self.hr_history.retain(|k, _| active_set.contains(k));
        self.motion_history.retain(|k, _| active_set.contains(k));
    }

    /// Get aggregate biometric entropy from all active devices.
    /// Combines entropy from multiple humans for block-level randomness.
    pub fn aggregate_entropy(&self) -> Vec<u8> {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        
        // Mix all recent HR values from all devices
        for (pubkey, hrs) in &self.hr_history {
            hasher.update(pubkey.as_bytes());
            for hr in hrs {
                hasher.update(hr.to_le_bytes());
            }
        }
        
        // Mix all motion data
        for (pubkey, motions) in &self.motion_history {
            hasher.update(pubkey.as_bytes());
            for m in motions {
                hasher.update(m.to_le_bytes());
            }
        }
        
        hasher.finalize().to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normal_heartbeat_passes() {
        let mut v = BiometricValidator::new();
        let result = v.validate("device1", 72, 0.1, 36.7);
        assert!(result.is_valid);
        assert!(result.confidence > 0.9);
    }

    #[test]
    fn test_extreme_temperature_reduces_confidence() {
        let mut v = BiometricValidator::new();
        let result = v.validate("device1", 72, 0.1, 45.0); // way too hot
        assert!(!result.is_valid || result.confidence < 0.5);
    }

    #[test]
    fn test_constant_hr_detected_as_synthetic() {
        let mut v = BiometricValidator::new();
        // Send 15 identical heartbeats — no HRV
        for _ in 0..15 {
            v.validate("device1", 72, 0.1, 36.7);
        }
        let result = v.validate("device1", 72, 0.1, 36.7);
        assert!(result.confidence < 0.5, "Constant HR should reduce confidence: {}", result.confidence);
    }

    #[test]
    fn test_natural_hrv_passes() {
        let mut v = BiometricValidator::new();
        // Simulate realistic HR and motion variability
        let hrs = [72, 74, 71, 75, 73, 70, 76, 72, 74, 71, 73, 75, 72, 74, 70];
        let motions = [0.08, 0.12, 0.09, 0.15, 0.11, 0.07, 0.13, 0.10, 0.14, 0.08, 0.12, 0.09, 0.11, 0.13, 0.10];
        for i in 0..hrs.len() {
            v.validate("device1", hrs[i], motions[i], 36.7);
        }
        let result = v.validate("device1", 73, 0.11, 36.7);
        assert!(result.is_valid);
        assert!(result.confidence > 0.7, "Natural HRV should have high confidence: {}", result.confidence);
    }

    #[test]
    fn test_entropy_extraction() {
        let mut v = BiometricValidator::new();
        let r1 = v.validate("device1", 72, 0.1, 36.7);
        let r2 = v.validate("device1", 73, 0.15, 36.8);
        
        // Entropy should be 32 bytes (SHA-256)
        assert_eq!(r1.entropy_bits.len(), 32);
        assert_eq!(r2.entropy_bits.len(), 32);
        
        // Different inputs should produce different entropy
        assert_ne!(r1.entropy_bits, r2.entropy_bits);
    }

    #[test]
    fn test_hr_motion_mismatch() {
        let mut v = BiometricValidator::new();
        // High HR with zero motion for many readings
        for _ in 0..15 {
            v.validate("device1", 160, 0.01, 36.7);
        }
        let result = v.validate("device1", 165, 0.01, 36.7);
        assert!(result.confidence < 0.7, "High HR + no motion should reduce confidence: {}", result.confidence);
    }
}
