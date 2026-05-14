import SwiftUI

public struct VoiceView: View {
    @State private var isListening = false
    @State private var transcript = ""

    public init() {}

    public var body: some View {
        VStack(spacing: 24) {
            Text("Voice Mode")
                .font(.largeTitle)

            Text(transcript)
                .font(.body)
                .padding()
                .frame(maxWidth: .infinity, minHeight: 80)
                .background(Color.gray.opacity(0.1))
                .cornerRadius(12)

            Button(action: toggleListening) {
                Image(systemName: isListening ? "mic.fill" : "mic")
                    .font(.system(size: 64))
                    .foregroundColor(isListening ? .red : .blue)
                    .frame(width: 120, height: 120)
                    .background(Circle().fill(Color.gray.opacity(0.2)))
            }

            Text(isListening ? "Listening..." : "Tap to speak")
                .foregroundColor(.secondary)

            Spacer()
        }
        .padding()
    }

    private func toggleListening() {
        isListening.toggle()
        if isListening {
            transcript = ""
        }
    }
}
