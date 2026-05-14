import SwiftUI

public struct MainView: View {
    public init() {}

    public var body: some View {
        TabView {
            ChatView()
                .tabItem {
                    Label("Chat", systemImage: "message.fill")
                }

            VoiceView()
                .tabItem {
                    Label("Voice", systemImage: "mic.fill")
                }

            CanvasView()
                .tabItem {
                    Label("Canvas", systemImage: "rectangle.grid.2x2")
                }
        }
    }
}
