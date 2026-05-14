// swift-tools-version:5.9
import PackageDescription

let package = Package(
    name: "SmartAssist",
    platforms: [.iOS(.v16), .macOS(.v13)],
    products: [
        .library(name: "SmartAssist", targets: ["SmartAssist"]),
    ],
    dependencies: [
        .package(url: "https://github.com/daltoniam/Starscream.git", from: "4.0.0"),
    ],
    targets: [
        .target(
            name: "SmartAssist",
            dependencies: ["Starscream"]
        ),
        .testTarget(
            name: "SmartAssistTests",
            dependencies: ["SmartAssist"]
        ),
    ]
)
