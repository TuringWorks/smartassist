import Foundation

public struct Device: Identifiable, Codable {
    public let id: String
    public var name: String
    public var deviceType: String
    public var pairedAt: Date?
    public var lastSeen: Date?
    public var connected: Bool

    public init(id: String, name: String, deviceType: String) {
        self.id = id
        self.name = name
        self.deviceType = deviceType
        self.pairedAt = nil
        self.lastSeen = nil
        self.connected = false
    }
}

public struct ChatMessage: Identifiable, Codable {
    public let id: String
    public let role: String
    public let content: String
    public let timestamp: Date

    public init(id: String, role: String, content: String) {
        self.id = id
        self.role = role
        self.content = content
        self.timestamp = Date()
    }
}

public struct Session: Identifiable, Codable {
    public let id: String
    public var status: String
    public var messages: [ChatMessage]
    public let createdAt: Date

    public init(id: String) {
        self.id = id
        self.status = "active"
        self.messages = []
        self.createdAt = Date()
    }
}
