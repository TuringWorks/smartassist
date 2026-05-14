/// SmartAssist iOS Companion App
///
/// Provides chat, voice, and canvas interfaces to the SmartAssist gateway.

import Foundation

public class SmartAssistApp {
    public static let version = "0.1.0"
    public static let shared = SmartAssistApp()

    public let gatewayClient = GatewayClient()
    public let voiceService = VoiceService()
    public let deviceStore = DeviceStore()

    private init() {}

    public func configure(gatewayUrl: URL) {
        gatewayClient.baseURL = gatewayUrl
    }
}
