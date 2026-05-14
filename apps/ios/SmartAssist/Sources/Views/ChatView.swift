import SwiftUI

public struct ChatView: View {
    @State private var messages: [ChatMessage] = []
    @State private var inputText = ""
    @State private var isSending = false
    @State private var errorMessage: String?

    public init() {}

    public var body: some View {
        VStack {
            List(messages) { message in
                HStack {
                    if message.role == "user" {
                        Spacer()
                        Text(message.content)
                            .padding()
                            .background(Color.blue.opacity(0.2))
                            .cornerRadius(8)
                    } else {
                        Text(message.content)
                            .padding()
                            .background(Color.gray.opacity(0.2))
                            .cornerRadius(8)
                        Spacer()
                    }
                }
            }

            if let error = errorMessage {
                Text(error)
                    .font(.caption)
                    .foregroundColor(.red)
                    .padding(.horizontal)
            }

            HStack {
                TextField("Message...", text: $inputText)
                    .textFieldStyle(RoundedBorderTextFieldStyle())
                    .disabled(isSending)

                Button(action: sendMessage) {
                    if isSending {
                        ProgressView()
                            .progressViewStyle(CircularProgressViewStyle())
                    } else {
                        Text("Send")
                    }
                }
                .disabled(inputText.isEmpty || isSending)
            }
            .padding()
        }
    }

    private func sendMessage() {
        let text = inputText.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty else { return }

        let userMessage = ChatMessage(id: UUID().uuidString, role: "user", content: text)
        messages.append(userMessage)
        inputText = ""
        isSending = true
        errorMessage = nil

        Task {
            do {
                let response = try await SmartAssistApp.shared.gatewayClient.sendRPC(
                    method: "chat",
                    params: [
                        "message": text,
                        "agent_id": "default"
                    ]
                )

                await MainActor.run {
                    isSending = false
                    if let result = response["result"] as? [String: Any],
                       let reply = result["text"] as? String {
                        let assistantMessage = ChatMessage(
                            id: UUID().uuidString,
                            role: "assistant",
                            content: reply
                        )
                        messages.append(assistantMessage)
                    } else {
                        errorMessage = "Unexpected response format"
                    }
                }
            } catch {
                await MainActor.run {
                    isSending = false
                    errorMessage = "Failed to send: \(error.localizedDescription)"
                }
            }
        }
    }
}
