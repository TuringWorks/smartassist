import SwiftUI

public struct DevicesView: View {
    @State private var devices: [Device] = []
    @State private var isPairing = false

    public init() {}

    public var body: some View {
        NavigationView {
            List(devices) { device in
                HStack {
                    Image(systemName: iconForType(device.deviceType))
                    VStack(alignment: .leading) {
                        Text(device.name)
                            .font(.headline)
                        Text(device.connected ? "Connected" : "Disconnected")
                            .font(.caption)
                            .foregroundColor(device.connected ? .green : .red)
                    }
                }
            }
            .navigationTitle("Paired Devices")
            .toolbar {
                Button("Pair") {
                    isPairing = true
                }
            }
            .sheet(isPresented: $isPairing) {
                PairingView()
            }
        }
    }

    private func iconForType(_ type: String) -> String {
        switch type {
        case "ios", "android": return "iphone"
        case "desktop": return "desktopcomputer"
        default: return "questionmark.circle"
        }
    }
}

public struct PairingView: View {
    @State private var deviceId = ""
    @State private var challengeCode = ""
    @State private var isLoading = false
    @Environment(\.dismiss) private var dismiss

    public init() {}

    public var body: some View {
        NavigationView {
            Form {
                Section("Device") {
                    TextField("Device ID", text: $deviceId)
                }
                Section {
                    Button("Request Pairing") {
                        Task {
                            await startPairing()
                        }
                    }
                    .disabled(deviceId.isEmpty || isLoading)
                }
                if !challengeCode.isEmpty {
                    Section("Challenge Code") {
                        Text(challengeCode)
                            .font(.system(.title, design: .monospaced))
                    }
                }
            }
            .navigationTitle("Pair Device")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
            }
        }
    }

    private func startPairing() async {
        isLoading = true
        defer { isLoading = false }

        do {
            let code = try await SmartAssistApp.shared.gatewayClient.pairDevice(
                deviceId: deviceId,
                name: "iPhone"
            )
            challengeCode = code
        } catch {
            challengeCode = "Error: \(error.localizedDescription)"
        }
    }
}
