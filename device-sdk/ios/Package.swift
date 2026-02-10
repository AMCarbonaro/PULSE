// swift-tools-version:5.9
import PackageDescription

let package = Package(
    name: "PulseSDK",
    platforms: [
        .iOS(.v15),
        .watchOS(.v8)
    ],
    products: [
        .library(
            name: "PulseSDK",
            targets: ["PulseSDK"]
        ),
    ],
    dependencies: [
        // Crypto for signing
        .package(url: "https://github.com/apple/swift-crypto.git", from: "3.0.0"),
    ],
    targets: [
        .target(
            name: "PulseSDK",
            dependencies: [
                .product(name: "Crypto", package: "swift-crypto"),
            ],
            path: "Sources"
        ),
        .testTarget(
            name: "PulseSDKTests",
            dependencies: ["PulseSDK"],
            path: "Tests"
        ),
    ]
)
