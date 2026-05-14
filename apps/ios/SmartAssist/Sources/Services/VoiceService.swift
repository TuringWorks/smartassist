import Foundation
import AVFoundation

public actor VoiceService {
    public var isRecording = false
    private var audioEngine: AVAudioEngine?

    public init() {}

    public func requestPermissions() async -> Bool {
        let status = AVCaptureDevice.authorizationStatus(for: .audio)
        switch status {
        case .authorized:
            return true
        case .notDetermined:
            return await AVCaptureDevice.requestAccess(for: .audio)
        default:
            return false
        }
    }

    public func startRecording() async throws {
        let session = AVAudioSession.sharedInstance()
        try session.setCategory(.playAndRecord, mode: .default, options: [.defaultToSpeaker])
        try session.setActive(true)
        isRecording = true
    }

    public func stopRecording() {
        isRecording = false
        audioEngine?.stop()
        audioEngine = nil
    }

    public func sendVoiceTrigger() async throws {
        // Send voice trigger to gateway via WebSocket
        // Placeholder for actual WebSocket implementation
    }
}
