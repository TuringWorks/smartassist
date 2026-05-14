import SwiftUI

public struct CanvasView: View {
    @State private var elements: [CanvasElement] = []

    public init() {}

    public var body: some View {
        VStack {
            Text("Canvas")
                .font(.headline)

            ScrollView {
                VStack(spacing: 12) {
                    ForEach(elements) { element in
                        CanvasElementView(element: element)
                    }
                }
                .padding()
            }
        }
    }
}

public struct CanvasElement: Identifiable {
    public let id = UUID()
    public var type: String
    public var content: String
}

public struct CanvasElementView: View {
    let element: CanvasElement

    public var body: some View {
        VStack(alignment: .leading) {
            Text(element.type)
                .font(.caption)
                .foregroundColor(.secondary)
            Text(element.content)
                .font(.body)
        }
        .padding()
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Color.white)
        .cornerRadius(8)
        .shadow(radius: 1)
    }
}
