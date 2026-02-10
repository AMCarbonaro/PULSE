import Foundation

/// Client for communicating with Pulse Network nodes
public class PulseNetworkClient {
    
    private let baseURL: URL
    private let session: URLSession
    private let encoder = JSONEncoder()
    private let decoder = JSONDecoder()
    
    public init(nodeURL: String) {
        self.baseURL = URL(string: nodeURL)!
        
        let config = URLSessionConfiguration.default
        config.timeoutIntervalForRequest = 10
        self.session = URLSession(configuration: config)
    }
    
    // MARK: - Heartbeat Submission
    
    /// Submit a signed heartbeat to the node
    public func submitHeartbeat(_ heartbeat: Heartbeat) async throws {
        let url = baseURL.appendingPathComponent("pulse")
        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = try encoder.encode(heartbeat)
        
        let (data, response) = try await session.data(for: request)
        
        guard let httpResponse = response as? HTTPURLResponse else {
            throw PulseError.invalidResponse
        }
        
        if httpResponse.statusCode != 200 {
            if let errorResponse = try? decoder.decode([String: String].self, from: data),
               let error = errorResponse["error"] {
                throw PulseError.networkError(error)
            }
            throw PulseError.networkError("HTTP \(httpResponse.statusCode)")
        }
    }
    
    // MARK: - Network Queries
    
    /// Get network statistics
    public func getStats() async throws -> NetworkStats {
        let url = baseURL.appendingPathComponent("stats")
        let (data, _) = try await session.data(from: url)
        
        let response = try decoder.decode(APIResponse<NetworkStats>.self, from: data)
        guard response.success, let stats = response.data else {
            throw PulseError.networkError(response.error ?? "Unknown error")
        }
        return stats
    }
    
    /// Get balance for a public key
    public func getBalance(pubkey: String) async throws -> Double {
        let url = baseURL.appendingPathComponent("balance/\(pubkey)")
        let (data, _) = try await session.data(from: url)
        
        struct BalanceResponse: Decodable {
            let pubkey: String
            let balance: Double
        }
        
        let response = try decoder.decode(APIResponse<BalanceResponse>.self, from: data)
        guard response.success, let balanceData = response.data else {
            throw PulseError.networkError(response.error ?? "Unknown error")
        }
        return balanceData.balance
    }
    
    /// Get chain info
    public func getChainInfo() async throws -> (height: UInt64, latestHash: String) {
        let url = baseURL.appendingPathComponent("chain")
        let (data, _) = try await session.data(from: url)
        
        struct ChainInfo: Decodable {
            let height: UInt64
            let latestHash: String
            let heartbeatPoolSize: Int
            
            enum CodingKeys: String, CodingKey {
                case height
                case latestHash = "latest_hash"
                case heartbeatPoolSize = "heartbeat_pool_size"
            }
        }
        
        let response = try decoder.decode(APIResponse<ChainInfo>.self, from: data)
        guard response.success, let info = response.data else {
            throw PulseError.networkError(response.error ?? "Unknown error")
        }
        return (info.height, info.latestHash)
    }
    
    /// Health check
    public func healthCheck() async throws -> Bool {
        let url = baseURL.appendingPathComponent("health")
        let (_, response) = try await session.data(from: url)
        
        guard let httpResponse = response as? HTTPURLResponse else {
            return false
        }
        return httpResponse.statusCode == 200
    }
}
