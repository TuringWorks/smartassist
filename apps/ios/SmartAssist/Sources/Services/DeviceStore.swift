import Foundation

public actor DeviceStore {
    private var devices: [Device] = []

    public init() {}

    public func allDevices() -> [Device] {
        devices
    }

    public func add(_ device: Device) {
        devices.append(device)
    }

    public func remove(id: String) {
        devices.removeAll { $0.id == id }
    }

    public func updateConnection(id: String, connected: Bool) {
        if let index = devices.firstIndex(where: { $0.id == id }) {
            devices[index].connected = connected
        }
    }
}
