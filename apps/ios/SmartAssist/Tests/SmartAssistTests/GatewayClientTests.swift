import XCTest
@testable import SmartAssist

final class GatewayClientTests: XCTestCase {
    func testDeviceInitialization() {
        let device = Device(id: "test-1", name: "Test Device", deviceType: "ios")
        XCTAssertEqual(device.id, "test-1")
        XCTAssertEqual(device.name, "Test Device")
        XCTAssertEqual(device.deviceType, "ios")
        XCTAssertFalse(device.connected)
    }

    func testChatMessageInitialization() {
        let message = ChatMessage(id: "msg-1", role: "user", content: "Hello")
        XCTAssertEqual(message.id, "msg-1")
        XCTAssertEqual(message.role, "user")
        XCTAssertEqual(message.content, "Hello")
    }

    func testSessionInitialization() {
        let session = Session(id: "session-1")
        XCTAssertEqual(session.id, "session-1")
        XCTAssertEqual(session.status, "active")
        XCTAssertTrue(session.messages.isEmpty)
    }

    func testSmartAssistAppVersion() {
        XCTAssertEqual(SmartAssistApp.version, "0.1.0")
    }
}
