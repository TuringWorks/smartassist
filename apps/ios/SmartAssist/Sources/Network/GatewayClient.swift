import Foundation

public actor GatewayClient {
    public var baseURL: URL?
    private var authToken: String?

    public init() {}

    public func setAuthToken(_ token: String) {
        self.authToken = token
    }

    public func sendRPC(method: String, params: [String: Any]? = nil) async throws -> [String: Any] {
        guard let baseURL = baseURL else {
            throw SmartAssistError.noGatewayURL
        }

        let url = baseURL.appendingPathComponent("rpc")
        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        if let token = authToken {
            request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        }

        let body: [String: Any] = [
            "jsonrpc": "2.0",
            "id": UUID().uuidString,
            "method": method,
            "params": params ?? [:]
        ]
        request.httpBody = try JSONSerialization.data(withJSONObject: body)

        let (data, response) = try await URLSession.shared.data(for: request)
        guard let httpResponse = response as? HTTPURLResponse, httpResponse.statusCode == 200 else {
            throw SmartAssistError.rpcFailed
        }

        guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            throw SmartAssistError.invalidResponse
        }

        return json
    }

    public func pairDevice(deviceId: String, name: String) async throws -> String {
        let result = try await sendRPC(
            method: "device.pair",
            params: ["device_id": deviceId, "name": name, "device_type": "ios"]
        )
        guard let challenge = result["result"] as? [String: Any],
              let code = challenge["challenge_code"] as? String else {
            throw SmartAssistError.pairingFailed
        }
        return code
    }

    public func listDevices() async throws -> [Device] {
        let result = try await sendRPC(method: "device.pair.list")
        guard let response = result["result"] as? [String: Any],
              let devices = response["devices"] as? [[String: Any]] else {
            return []
        }
        return devices.compactMap { dict in
            guard let id = dict["id"] as? String,
                  let name = dict["name"] as? String,
                  let type = dict["device_type"] as? String else { return nil }
            var device = Device(id: id, name: name, deviceType: type)
            device.connected = dict["connected"] as? Bool ?? false
            return device
        }
    }
}

public enum SmartAssistError: Error {
    case noGatewayURL
    case rpcFailed
    case invalidResponse
    case pairingFailed
}
