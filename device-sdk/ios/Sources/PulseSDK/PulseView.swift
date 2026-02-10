#if canImport(SwiftUI) && (os(iOS) || os(watchOS))
import SwiftUI

/// A ready-to-use SwiftUI view for pulsing
public struct PulseView: View {
    @ObservedObject var client: PulseClient
    @State private var networkStats: NetworkStats?
    @State private var showingError = false
    
    public init(client: PulseClient) {
        self.client = client
    }
    
    public var body: some View {
        ScrollView {
            VStack(spacing: 24) {
                // Status Header
                statusHeader
                
                // Heart Rate Display
                heartRateDisplay
                
                // Balance
                balanceDisplay
                
                // Controls
                controlButtons
                
                // Network Stats
                if let stats = networkStats {
                    networkStatsView(stats)
                }
                
                // Public Key
                publicKeyView
            }
            .padding()
        }
        .task {
            await connect()
        }
        .alert("Error", isPresented: $showingError) {
            Button("OK") { }
        } message: {
            Text(client.lastError ?? "Unknown error")
        }
        .onChange(of: client.lastError) { error in
            showingError = error != nil
        }
    }
    
    // MARK: - Subviews
    
    private var statusHeader: some View {
        HStack {
            Circle()
                .fill(client.statusColor)
                .frame(width: 12, height: 12)
            Text(client.statusText)
                .font(.headline)
            Spacer()
        }
    }
    
    private var heartRateDisplay: some View {
        VStack(spacing: 8) {
            Text("❤️")
                .font(.system(size: 60))
                .scaleEffect(client.isPulsing ? 1.1 : 1.0)
                .animation(.easeInOut(duration: 0.5).repeatForever(autoreverses: true), value: client.isPulsing)
            
            Text("\(Int(client.currentHeartRate))")
                .font(.system(size: 72, weight: .bold, design: .rounded))
            
            Text("BPM")
                .font(.caption)
                .foregroundColor(.secondary)
        }
        .padding()
        .background(Color(.systemBackground))
        .cornerRadius(16)
        .shadow(radius: 4)
    }
    
    private var balanceDisplay: some View {
        VStack(spacing: 4) {
            Text("Balance")
                .font(.caption)
                .foregroundColor(.secondary)
            Text(client.formattedBalance)
                .font(.title2.bold())
        }
        .padding()
        .frame(maxWidth: .infinity)
        .background(Color(.secondarySystemBackground))
        .cornerRadius(12)
    }
    
    private var controlButtons: some View {
        HStack(spacing: 16) {
            Button {
                if client.isPulsing {
                    client.stopPulsing()
                } else {
                    client.startPulsing()
                }
            } label: {
                Label(
                    client.isPulsing ? "Stop" : "Start Pulsing",
                    systemImage: client.isPulsing ? "stop.circle.fill" : "heart.circle.fill"
                )
                .frame(maxWidth: .infinity)
                .padding()
                .background(client.isPulsing ? Color.red : Color.green)
                .foregroundColor(.white)
                .cornerRadius(12)
            }
            
            Button {
                Task {
                    await refreshStats()
                }
            } label: {
                Image(systemName: "arrow.clockwise")
                    .padding()
                    .background(Color(.tertiarySystemBackground))
                    .cornerRadius(12)
            }
        }
    }
    
    private func networkStatsView(_ stats: NetworkStats) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Network")
                .font(.headline)
            
            HStack {
                StatItem(title: "Chain", value: "\(stats.chainLength) blocks")
                StatItem(title: "Minted", value: String(format: "%.0f", stats.totalMinted))
                StatItem(title: "Accounts", value: "\(stats.activeAccounts)")
            }
            
            HStack {
                StatItem(title: "Security", value: String(format: "%.2f", stats.totalSecurity))
                StatItem(title: "Block Time", value: String(format: "%.1fs", stats.avgBlockTime))
            }
        }
        .padding()
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Color(.secondarySystemBackground))
        .cornerRadius(12)
    }
    
    private var publicKeyView: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text("Your Identity")
                .font(.caption)
                .foregroundColor(.secondary)
            
            Text(client.publicKey.prefix(32) + "...")
                .font(.system(.caption, design: .monospaced))
                .foregroundColor(.secondary)
        }
        .padding()
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Color(.tertiarySystemBackground))
        .cornerRadius(8)
    }
    
    // MARK: - Actions
    
    private func connect() async {
        do {
            try await client.requestAuthorization()
            try await client.connect()
            await refreshStats()
        } catch {
            // Handle error
        }
    }
    
    private func refreshStats() async {
        networkStats = try? await client.getNetworkStats()
        await client.refreshBalance()
    }
}

// MARK: - Helper Views

private struct StatItem: View {
    let title: String
    let value: String
    
    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(title)
                .font(.caption2)
                .foregroundColor(.secondary)
            Text(value)
                .font(.caption.bold())
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

// MARK: - Preview

#if DEBUG
struct PulseView_Previews: PreviewProvider {
    static var previews: some View {
        PulseView(client: try! PulseClient(nodeURL: "http://localhost:8080"))
    }
}
#endif

#endif
